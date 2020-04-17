//! `signature_checker` module provides a detached thread routine
//! dedicated for checking the signatures of incoming transactions.
//! Main routine of this module operates a multithreaded event loop,
//! which is used to spawn concurrent tasks to efficiently check the
//! transactions signatures.

// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt, StreamExt,
};
use tokio::runtime::{Builder, Handle};
// Workspace uses
use models::{
    config_options::ThreadPanicNotify,
    node::{tx::TxEthSignature, FranklinTx},
};
// Local uses
use crate::eth_watch::EthWatchRequest;
use crate::mempool::TxAddError;

/// Wrapper on a `FranklinTx` which guarantees that
/// transaction was checked and signatures associated with
/// this transactions are correct.
///
/// Underlying `FranklinTx` is a private field, thus no such
/// object can be created without verification.
#[derive(Debug)]
pub struct VerifiedTx(FranklinTx);

impl VerifiedTx {
    /// Checks the transaction correctness by verifying its
    /// Ethereum signature (if required) and `ZKSync` signature.
    pub async fn verify(
        request: &VerifyTxSignatureRequest,
        eth_watch_req: mpsc::Sender<EthWatchRequest>,
    ) -> Result<Self, TxAddError> {
        verify_eth_signature(&request, eth_watch_req)
            .await
            .and_then(|_| verify_tx_correctness(request.tx.clone()))
            .map(Self)
    }

    /// Takes the `FranklinTx` out of the wrapper.
    pub fn into_inner(self) -> FranklinTx {
        self.0
    }
}

/// Verifies the Ethereum signature of the transaction.
async fn verify_eth_signature(
    request: &VerifyTxSignatureRequest,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
) -> Result<(), TxAddError> {
    // Check if the tx is a `ChangePubKey` operation without an Ethereum signature.
    if let FranklinTx::ChangePubKey(change_pk) = &request.tx {
        if change_pk.eth_signature.is_none() {
            // Check that user is allowed to perform this operation.
            let eth_watch_resp = oneshot::channel();
            eth_watch_req
                .clone()
                .send(EthWatchRequest::IsPubkeyChangeAuthorized {
                    address: change_pk.account,
                    nonce: change_pk.nonce,
                    pubkey_hash: change_pk.new_pk_hash.clone(),
                    resp: eth_watch_resp.0,
                })
                .await
                .expect("ETH watch req receiver dropped");

            let is_authorized = eth_watch_resp.1.await.expect("Err response from eth watch");
            if !is_authorized {
                return Err(TxAddError::ChangePkNotAuthorized);
            }
        }
    }

    // Check the signature.
    if let Some((signature, message)) = &request.eth_sign_data {
        match &signature {
            TxEthSignature::EthereumSignature(packed_signature) => {
                let signer_account = packed_signature
                    .signature_recover_signer(message.as_bytes())
                    .or(Err(TxAddError::IncorrectEthSignature))?;

                if signer_account != request.tx.account() {
                    return Err(TxAddError::IncorrectEthSignature);
                }
            }
            TxEthSignature::EIP1271Signature(signature) => {
                let message = format!("\x19Ethereum Signed Message:\n{}{}", message.len(), message);

                let eth_watch_resp = oneshot::channel();
                eth_watch_req
                    .clone()
                    .send(EthWatchRequest::CheckEIP1271Signature {
                        address: request.tx.account(),
                        message: message.into_bytes(),
                        signature: signature.clone(),
                        resp: eth_watch_resp.0,
                    })
                    .await
                    .expect("ETH watch req receiver dropped");

                let signature_correct = eth_watch_resp
                    .1
                    .await
                    .expect("Failed receiving response from eth watch")
                    .map_err(|e| warn!("Err in eth watch: {}", e))
                    .or(Err(TxAddError::EIP1271SignatureVerificationFail))?;

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
fn verify_tx_correctness(mut tx: FranklinTx) -> Result<FranklinTx, TxAddError> {
    if !tx.check_correctness() {
        return Err(TxAddError::IncorrectTx);
    }

    Ok(tx)
}

/// Request for the signature check.
#[derive(Debug)]
pub struct VerifyTxSignatureRequest {
    pub tx: FranklinTx,
    /// `eth_sign_data` is a tuple of the Ethereum signature and the message
    /// which user should have signed with their private key.
    /// Can be `None` if the Ethereum signature is not required.
    pub eth_sign_data: Option<(TxEthSignature, String)>,
    /// Channel for sending the check response.
    pub response: oneshot::Sender<Result<VerifiedTx, TxAddError>>,
}

/// Main routine of the concurrent signature checker.
/// See the module documentation for details.
pub fn start_sign_checker_detached(
    input: mpsc::Receiver<VerifyTxSignatureRequest>,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    panic_notify: mpsc::Sender<bool>,
) {
    /// Main signature check requests handler.
    /// Basically it receives the requests through the channel and verifies signatures,
    /// notifying the request sender about the check result.
    async fn checker_routine(
        handle: Handle,
        mut input: mpsc::Receiver<VerifyTxSignatureRequest>,
        eth_watch_req: mpsc::Sender<EthWatchRequest>,
    ) {
        while let Some(request) = input.next().await {
            let eth_watch_req = eth_watch_req.clone();
            handle.spawn(async move {
                let resp = VerifiedTx::verify(&request, eth_watch_req).await;

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
            runtime.block_on(checker_routine(handle, input, eth_watch_req));
        })
        .expect("failed to start signature checker thread");
}
