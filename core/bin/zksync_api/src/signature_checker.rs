//! `signature_checker` module provides a detached thread routine
//! dedicated for checking the signatures of incoming transactions.
//! Main routine of this module operates a multithreaded event loop,
//! which is used to spawn concurrent tasks to efficiently check the
//! transactions signatures.

// External uses
use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use tokio::runtime::{Builder, Handle};
// Workspace uses
use zksync_types::{tx::TxEthSignature, SignedZkSyncTx, ZkSyncTx};
// Local uses
use crate::{eth_checker::EthereumChecker, tx_error::TxAddError};
use zksync_config::ConfigurationOptions;
use zksync_types::tx::EthSignData;
use zksync_utils::panic_notify::ThreadPanicNotify;

/// Wrapper on a `ZkSyncTx` which guarantees that
/// transaction was checked and signatures associated with
/// this transactions are correct.
///
/// Underlying `ZkSyncTx` is a private field, thus no such
/// object can be created without verification.
#[derive(Debug, Clone)]
pub struct VerifiedTx(SignedZkSyncTx);

impl VerifiedTx {
    /// Checks the transaction correctness by verifying its
    /// Ethereum signature (if required) and `ZKSync` signature.
    pub async fn verify(
        request: &VerifyTxSignatureRequest,
        eth_checker: &EthereumChecker<web3::transports::Http>,
    ) -> Result<Self, TxAddError> {
        verify_eth_signature(&request, eth_checker)
            .await
            .and_then(|_| verify_tx_correctness(request.tx.clone()))
            .map(|tx| {
                Self(SignedZkSyncTx {
                    tx,
                    eth_sign_data: request.eth_sign_data.clone(),
                })
            })
    }

    /// Takes the `ZkSyncTx` out of the wrapper.
    pub fn into_inner(self) -> SignedZkSyncTx {
        self.0
    }

    /// Takes reference to the inner `ZkSyncTx`.
    pub fn inner(&self) -> &SignedZkSyncTx {
        &self.0
    }
}

/// Verifies the Ethereum signature of the transaction.
async fn verify_eth_signature(
    request: &VerifyTxSignatureRequest,
    eth_checker: &EthereumChecker<web3::transports::Http>,
) -> Result<(), TxAddError> {
    // Check if the tx is a `ChangePubKey` operation without an Ethereum signature.
    if let ZkSyncTx::ChangePubKey(change_pk) = &request.tx {
        if change_pk.eth_signature.is_none() {
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
    if let Some(sign_data) = &request.eth_sign_data {
        match &sign_data.signature {
            TxEthSignature::EthereumSignature(packed_signature) => {
                let signer_account = packed_signature
                    .signature_recover_signer(sign_data.message.as_bytes())
                    .or(Err(TxAddError::IncorrectEthSignature))?;

                if signer_account != request.tx.account() {
                    return Err(TxAddError::IncorrectEthSignature);
                }
            }
            TxEthSignature::EIP1271Signature(signature) => {
                let message = format!(
                    "\x19Ethereum Signed Message:\n{}{}",
                    sign_data.message.len(),
                    &sign_data.message
                );

                let signature_correct = eth_checker
                    .is_eip1271_signature_correct(
                        request.tx.account(),
                        message.into_bytes(),
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

    Ok(())
}

/// Verifies the correctness of the ZKSync transaction (including the
/// signature check).
fn verify_tx_correctness(mut tx: ZkSyncTx) -> Result<ZkSyncTx, TxAddError> {
    if !tx.check_correctness() {
        return Err(TxAddError::IncorrectTx);
    }

    Ok(tx)
}

/// Request for the signature check.
#[derive(Debug)]
pub struct VerifyTxSignatureRequest {
    pub tx: ZkSyncTx,
    /// `eth_sign_data` is a tuple of the Ethereum signature and the message
    /// which user should have signed with their private key.
    /// Can be `None` if the Ethereum signature is not required.
    pub eth_sign_data: Option<EthSignData>,
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
        while let Some(request) = input.next().await {
            let eth_checker = eth_checker.clone();
            handle.spawn(async move {
                let resp = VerifiedTx::verify(&request, &eth_checker).await;

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
