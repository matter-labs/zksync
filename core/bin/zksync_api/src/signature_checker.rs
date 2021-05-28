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
    tx::{EthBatchSignData, EthSignData, TxEthSignature},
    Address, Order, SignedZkSyncTx, Token, ZkSyncTx,
};
// Local uses
use crate::{eth_checker::EthereumChecker, tx_error::TxAddError};
use zksync_eth_client::EthereumGateway;
use zksync_utils::panic_notify::ThreadPanicNotify;

/// `TxVariant` is used to form a verify request. It is possible to wrap
/// either a single transaction, or the transaction batch.
#[derive(Debug, Clone)]
pub enum TxVariant {
    Tx(SignedZkSyncTx),
    Batch(Vec<SignedZkSyncTx>, Option<EthBatchSignData>),
    Order(Box<Order>),
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
        request_data: RequestData,
        eth_checker: &EthereumChecker,
    ) -> Result<Self, TxAddError> {
        verify_eth_signature(&request_data, eth_checker).await?;
        let mut tx_variant = request_data.get_tx_variant();
        verify_tx_correctness(&mut tx_variant)?;

        Ok(Self(tx_variant))
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
            TxVariant::Order(_) => panic!("called `unwrap_tx` on an `Order` value"),
        }
    }

    /// Takes the Vec of `SignedZkSyncTx` and the verified signature data out of the wrapper.
    pub fn unwrap_batch(self) -> (Vec<SignedZkSyncTx>, Option<EthBatchSignData>) {
        match self.0 {
            TxVariant::Batch(txs, batch_sign_data) => (txs, batch_sign_data),
            TxVariant::Tx(_) => panic!("called `unwrap_batch` on a `Tx` value"),
            TxVariant::Order(_) => panic!("called `unwrap_batch` on an `Order` value"),
        }
    }
}

/// Verifies the Ethereum signature of the (batch of) transaction(s).
async fn verify_eth_signature(
    request_data: &RequestData,
    eth_checker: &EthereumChecker,
) -> Result<(), TxAddError> {
    match request_data {
        RequestData::Tx(request) => {
            verify_eth_signature_single_tx(
                &request.tx,
                request.sender,
                request.token.clone(),
                eth_checker,
            )
            .await?;
        }
        RequestData::Batch(request) => {
            let accounts = &request.senders;
            let tokens = &request.tokens;
            let txs = &request.txs;

            if accounts.len() != request.txs.len() {
                return Err(TxAddError::Other);
            }
            if let Some(batch_sign_data) = &request.batch_sign_data {
                verify_eth_signature_txs_batch(txs, accounts, batch_sign_data, eth_checker).await?;
            }
            // In case there're signatures provided for some of transactions
            // we still verify them.
            for ((tx, &account), token) in
                txs.iter().zip(accounts.iter()).zip(tokens.iter().cloned())
            {
                verify_eth_signature_single_tx(tx, account, token, eth_checker).await?;
            }
        }
        RequestData::Order(request) => {
            let signature_correct = verify_ethereum_signature(
                &request.sign_data.signature,
                &request.sign_data.message,
                request.sender,
                eth_checker,
            )
            .await;
            if !signature_correct {
                return Err(TxAddError::IncorrectEthSignature);
            }
        }
    }

    Ok(())
}

/// Given a single Ethereum signature and a message, checks that it
/// was signed by an expected address.
async fn verify_ethereum_signature(
    eth_signature: &TxEthSignature,
    message: &[u8],
    sender_address: Address,
    eth_checker: &EthereumChecker,
) -> bool {
    let signer_account = match eth_signature {
        TxEthSignature::EthereumSignature(packed_signature) => {
            packed_signature.signature_recover_signer(message)
        }
        TxEthSignature::EIP1271Signature(signature) => {
            return eth_checker
                .is_eip1271_signature_correct(sender_address, message, signature.clone())
                .await
                .expect("Unable to check EIP1271 signature")
        }
    };
    match signer_account {
        Ok(address) => address == sender_address,
        Err(_) => false,
    }
}

async fn verify_eth_signature_single_tx(
    tx: &SignedZkSyncTx,
    sender_address: Address,
    token: Token,
    eth_checker: &EthereumChecker,
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
        let signature = &sign_data.signature;
        let mut signature_correct =
            verify_ethereum_signature(signature, &sign_data.message, sender_address, eth_checker)
                .await;
        if !signature_correct {
            let old_message = tx.get_old_ethereum_sign_message(token);
            if let Some(message) = old_message {
                signature_correct = verify_ethereum_signature(
                    signature,
                    message.as_bytes(),
                    sender_address,
                    eth_checker,
                )
                .await;
            }
        }
        if !signature_correct {
            return Err(TxAddError::IncorrectEthSignature);
        }
    }

    metrics::histogram!(
        "signature_checker.verify_eth_signature_single_tx",
        start.elapsed()
    );
    Ok(())
}

async fn verify_eth_signature_txs_batch(
    txs: &[SignedZkSyncTx],
    senders: &[Address],
    batch_sign_data: &EthBatchSignData,
    eth_checker: &EthereumChecker,
) -> Result<(), TxAddError> {
    let start = Instant::now();
    // Cache for verified senders.
    let mut signers = HashSet::with_capacity(senders.len());
    // For every sender check whether there exists at least one signature that matches it.
    let old_message = match txs.iter().all(|tx| tx.is_backwards_compatible()) {
        true => Some(EthBatchSignData::get_old_ethereum_batch_message(
            txs.iter().map(|tx| &tx.tx),
        )),
        false => None,
    };

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
            let mut signature_correct = verify_ethereum_signature(
                signature,
                &batch_sign_data.message,
                *sender,
                eth_checker,
            )
            .await;
            if !signature_correct {
                if let Some(old_message) = &old_message {
                    signature_correct = verify_ethereum_signature(
                        signature,
                        old_message.as_slice(),
                        *sender,
                        eth_checker,
                    )
                    .await;
                }
            }
            if signature_correct {
                signers.insert(sender);
                sender_correct = true;
                break;
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
        TxVariant::Order(order) => {
            if !order.check_correctness() {
                return Err(TxAddError::IncorrectTx);
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct TxRequest {
    pub tx: SignedZkSyncTx,
    /// Sender of transaction. This field is needed since for `ForcedExit` account affected by
    /// the transaction and actual sender can be different. Thus, we require request sender to
    /// perform a database query and fetch actual addresses if necessary.
    pub sender: Address,
    /// Resolved token might be used to obtain old-formatted 2-FA messages.
    /// Needed for backwards compatibility.
    pub token: Token,
}

#[derive(Debug)]
pub struct BatchRequest {
    pub txs: Vec<SignedZkSyncTx>,
    pub batch_sign_data: Option<EthBatchSignData>,
    pub senders: Vec<Address>,
    pub tokens: Vec<Token>,
}

#[derive(Debug)]
pub struct OrderRequest {
    pub order: Box<Order>,
    pub sign_data: EthSignData,
    pub sender: Address,
}

/// Request for the signature check.
#[derive(Debug)]
pub struct VerifySignatureRequest {
    pub data: RequestData,
    /// Channel for sending the check response.
    pub response: oneshot::Sender<Result<VerifiedTx, TxAddError>>,
}

#[derive(Debug)]
pub enum RequestData {
    Tx(TxRequest),
    Batch(BatchRequest),
    Order(OrderRequest),
}

impl RequestData {
    pub fn get_tx_variant(&self) -> TxVariant {
        match &self {
            RequestData::Tx(request) => TxVariant::Tx(request.tx.clone()),
            RequestData::Batch(request) => {
                TxVariant::Batch(request.txs.clone(), request.batch_sign_data.clone())
            }
            RequestData::Order(request) => TxVariant::Order(request.order.clone()),
        }
    }
}

/// Main routine of the concurrent signature checker.
/// See the module documentation for details.
pub fn start_sign_checker_detached(
    client: EthereumGateway,
    input: mpsc::Receiver<VerifySignatureRequest>,
    panic_notify: mpsc::Sender<bool>,
) {
    let eth_checker = EthereumChecker::new(client);

    /// Main signature check requests handler.
    /// Basically it receives the requests through the channel and verifies signatures,
    /// notifying the request sender about the check result.
    async fn checker_routine(
        handle: Handle,
        mut input: mpsc::Receiver<VerifySignatureRequest>,
        eth_checker: EthereumChecker,
    ) {
        while let Some(VerifySignatureRequest { data, response }) = input.next().await {
            let eth_checker = eth_checker.clone();
            handle.spawn(async move {
                let resp = VerifiedTx::verify(data, &eth_checker).await;

                response.send(resp).unwrap_or_default();
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
