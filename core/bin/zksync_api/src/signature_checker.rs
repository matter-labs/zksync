//! `signature_checker` module provides a detached thread routine
//! dedicated for checking the signatures of incoming transactions.
//! Main routine of this module operates a multithreaded event loop,
//! which is used to spawn concurrent tasks to efficiently check the
//! transactions signatures.

// Built-in uses
use std::collections::HashSet;
use std::time::Instant;

// External uses
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use tokio::runtime::{Builder, Handle};
// Workspace uses
use zksync_types::{
    tx::{BatchSignData, TxEthSignature},
    Address, SignedZkSyncTx, ZkSyncTx,
};
// Local uses
use crate::{eth_checker::EthereumChecker, tx_error::TxAddError};
use zksync_config::ConfigurationOptions;
use zksync_utils::panic_notify::ThreadPanicNotify;

/// `TxVariant` is used to form a verify request. It is possible to wrap
/// either a single transaction, or the transaction batch.
#[derive(Debug, Clone)]
pub enum TxVariant {
    Tx(SignedZkSyncTx),
    Batch(Vec<SignedZkSyncTx>, BatchSignData),
}

/// Wrapper on a `TxVariant` which guarantees that (a batch of)
/// transaction(s) was checked and signatures associated with
/// this transactions are correct.
///
/// Underlying `TxVariant` is a private field, thus no such
/// object can be created without verification.
#[derive(Debug, Clone)]
pub struct VerifiedTx(TxVariant);

impl VerifiedTx {
    /// Checks the (batch of) transaction(s) correctness by verifying its
    /// Ethereum signature (if required) and `ZKSync` signature.
    pub async fn verify(
        request: &mut VerifyTxSignatureRequest,
        eth_checker: &EthereumChecker<web3::transports::Http>,
    ) -> Result<Self, TxAddError> {
        verify_eth_signature(request, eth_checker).await?;
        verify_tx_correctness(&mut request.tx)?;

        Ok(Self(request.tx.clone()))
    }

    /// Creates a verified wrapper without actually verifying the original data.
    #[cfg(test)]
    pub(crate) fn unverified(inner: TxVariant) -> Self {
        Self(inner)
    }

    /// Takes the `TxVariant` out of the wrapper.
    pub fn unwrap_tx(self) -> SignedZkSyncTx {
        match self.0 {
            TxVariant::Tx(tx) => tx,
            TxVariant::Batch(_, _) => panic!("called `unwrap_tx` on a `Batch` value"),
        }
    }

    /// Takes the Vec of `SignedZkSyncTx` and the verified signature data out of the wrapper.
    pub fn unwrap_batch(self) -> (Vec<SignedZkSyncTx>, BatchSignData) {
        match self.0 {
            TxVariant::Batch(txs, batch_sign_data) => (txs, batch_sign_data),
            TxVariant::Tx(_) => panic!("called `unwrap_batch` on a `Tx` value"),
        }
    }
}

/// Verifies the Ethereum signature of the (batch of) transaction(s).
async fn verify_eth_signature(
    request: &VerifyTxSignatureRequest,
    eth_checker: &EthereumChecker<web3::transports::Http>,
) -> Result<(), TxAddError> {
    let accounts = &request.senders;

    match &request.tx {
        TxVariant::Tx(tx) => {
            if accounts.len() != 1 {
                return Err(TxAddError::Other);
            }
            verify_eth_signature_single_tx(tx, accounts[0], eth_checker).await?;
        }
        TxVariant::Batch(txs, batch_sign_data) => {
            if accounts.len() != txs.len() {
                return Err(TxAddError::Other);
            }
            verify_eth_signature_txs_batch(accounts, batch_sign_data, eth_checker).await?;
            // In case there're signatures provided for some of transactions
            // we still verify them.
            for (tx, &account) in txs.iter().zip(accounts.iter()) {
                verify_eth_signature_single_tx(tx, account, eth_checker).await?;
            }
        }
    }

    Ok(())
}

async fn verify_eth_signature_single_tx(
    tx: &SignedZkSyncTx,
    sender_address: Address,
    eth_checker: &EthereumChecker<web3::transports::Http>,
) -> Result<(), TxAddError> {
    let start = Instant::now();
    // Check if the tx is a `ChangePubKey` operation without an Ethereum signature.
    if let ZkSyncTx::ChangePubKey(change_pk) = &tx.tx {
        if change_pk.is_onchain() {
            // Check that user is allowed to perform this operation.
            let is_authorized = eth_checker
                .is_new_pubkey_hash_authorized(
                    change_pk.account,
                    change_pk.nonce,
                    &change_pk.new_pk_hash,
                )
                .await
                .expect("Unable to check onchain ChangePubKey Authorization");

            if !is_authorized {
                return Err(TxAddError::ChangePkNotAuthorized);
            }
        }
    }

    // Check the signature.
    if let Some(sign_data) = &tx.eth_sign_data {
        match &sign_data.signature {
            TxEthSignature::EthereumSignature(packed_signature) => {
                let signer_account = packed_signature
                    .signature_recover_signer(&sign_data.message)
                    .or(Err(TxAddError::IncorrectEthSignature))?;

                if signer_account != sender_address {
                    return Err(TxAddError::IncorrectEthSignature);
                }
            }
            TxEthSignature::EIP1271Signature(signature) => {
                let signature_correct = eth_checker
                    .is_eip1271_signature_correct(
                        sender_address,
                        &sign_data.message,
                        signature.clone(),
                    )
                    .await
                    .expect("Unable to check EIP1271 signature");

                if !signature_correct {
                    return Err(TxAddError::IncorrectTx);
                }
            }
        };
    }

    metrics::histogram!(
        "signature_checker.verify_eth_signature_single_tx",
        start.elapsed()
    );
    Ok(())
}

async fn verify_eth_signature_txs_batch(
    senders: &[Address],
    batch_sign_data: &BatchSignData,
    eth_checker: &EthereumChecker<web3::transports::Http>,
) -> Result<(), TxAddError> {
    let start = Instant::now();
    // Cache for verified senders.
    let mut signers = HashSet::with_capacity(senders.len());
    // For every sender check whether there exists at least one signature that matches it.
    for sender in senders {
        if signers.contains(sender) {
            continue;
        }
        // All possible signers are cached already and this sender didn't match any of them.
        if signers.len() == batch_sign_data.signatures.len() {
            return Err(TxAddError::IncorrectEthSignature);
        }
        // This block will set the `sender_correct` variable to `true` at the first match.
        let mut sender_correct = false;
        for signature in &batch_sign_data.signatures {
            match signature {
                TxEthSignature::EthereumSignature(packed_signature) => {
                    let signer_account = packed_signature
                        .signature_recover_signer(&batch_sign_data.message)
                        .or(Err(TxAddError::IncorrectEthSignature))?;
                    // Always cache it as it's correct one.
                    signers.insert(signer_account);
                    if sender == &signer_account {
                        sender_correct = true;
                        break;
                    }
                }
                TxEthSignature::EIP1271Signature(signature) => {
                    let signature_correct = eth_checker
                        .is_eip1271_signature_correct(
                            *sender,
                            &batch_sign_data.message,
                            signature.clone(),
                        )
                        .await
                        .expect("Unable to check EIP1271 signature");
                    if signature_correct {
                        signers.insert(*sender);
                        sender_correct = true;
                        break;
                    }
                }
            }
        }
        // No signature for this transaction found, return error.
        if !sender_correct {
            return Err(TxAddError::IncorrectEthSignature);
        }
    }
    metrics::histogram!(
        "signature_checker.verify_eth_signature_txs_batch",
        start.elapsed()
    );
    Ok(())
}

/// Verifies the correctness of the ZKSync transaction(s) (including the
/// signature check).
fn verify_tx_correctness(tx: &mut TxVariant) -> Result<(), TxAddError> {
    match tx {
        TxVariant::Tx(tx) => {
            if !tx.tx.check_correctness() {
                return Err(TxAddError::IncorrectTx);
            }
        }
        TxVariant::Batch(batch, _) => {
            if batch.iter_mut().any(|tx| !tx.tx.check_correctness()) {
                return Err(TxAddError::IncorrectTx);
            }
        }
    }
    Ok(())
}

/// Request for the signature check.
#[derive(Debug)]
pub struct VerifyTxSignatureRequest {
    pub tx: TxVariant,
    /// Senders of transactions. This field is needed since for `ForcedExit` account affected by
    /// the transaction and actual sender can be different. Thus, we require request sender to
    /// perform a database query and fetch actual addresses if necessary.
    pub senders: Vec<Address>,
    /// Channel for sending the check response.
    pub response: oneshot::Sender<Result<VerifiedTx, TxAddError>>,
}

/// Main routine of the concurrent signature checker.
/// See the module documentation for details.
pub fn start_sign_checker_detached(
    config_options: ConfigurationOptions,
    input: mpsc::Receiver<VerifyTxSignatureRequest>,
    panic_notify: mpsc::Sender<bool>,
) {
    let transport = web3::transports::Http::new(&config_options.web3_url).unwrap();
    let web3 = web3::Web3::new(transport);

    let eth_checker = EthereumChecker::new(web3, config_options.contract_eth_addr);

    /// Main signature check requests handler.
    /// Basically it receives the requests through the channel and verifies signatures,
    /// notifying the request sender about the check result.
    async fn checker_routine(
        handle: Handle,
        mut input: mpsc::Receiver<VerifyTxSignatureRequest>,
        eth_checker: EthereumChecker<web3::transports::Http>,
    ) {
        while let Some(mut request) = input.next().await {
            let eth_checker = eth_checker.clone();
            handle.spawn(async move {
                let resp = VerifiedTx::verify(&mut request, &eth_checker).await;

                request.response.send(resp).unwrap_or_default();
            });
        }
    }

    std::thread::Builder::new()
        .name("Signature checker thread".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

            let mut runtime = Builder::new()
                .enable_all()
                .threaded_scheduler()
                .build()
                .expect("failed to build runtime for signature processor");
            let handle = runtime.handle().clone();
            runtime.block_on(checker_routine(handle, input, eth_checker));
        })
        .expect("failed to start signature checker thread");
}
