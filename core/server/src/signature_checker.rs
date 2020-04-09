// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt, StreamExt,
};
use tokio::runtime::{Builder, Handle};
// Workspace uses
use models::{
    config_options::ThreadPanicNotify,
    node::{tx::TxEthSignature, Address, FranklinTx},
};
// Local uses
// use crate::api_server::rpc_server::RpcErrorCodes;
use crate::eth_watch::EthWatchRequest;
use crate::mempool::TxAddError;

async fn verify_eth_signature(
    request: &VerifyTxSignatureRequest,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
) -> Result<(), TxAddError> {
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
                let eth_watch_resp = oneshot::channel();
                eth_watch_req
                    .clone()
                    .send(EthWatchRequest::CheckEIP1271Signature {
                        address: request.tx.account(),
                        data: message.as_bytes().to_vec(),
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

fn verify_tx_correctness(tx: &FranklinTx) -> Result<(), TxAddError> {
    if !tx.check_correctness() {
        return Err(TxAddError::IncorrectTx);
    }

    Ok(())
}

#[derive(Debug)]
pub struct SignatureCheckResponse {
    pub resolved_address: Result<Address, failure::Error>,
    pub tx_correct: bool,
}

#[derive(Debug)]
pub struct VerifyTxSignatureRequest {
    pub tx: FranklinTx,
    pub eth_sign_data: Option<(TxEthSignature, String)>,
    pub response: oneshot::Sender<Result<(), TxAddError>>,
}

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
        while let Some(req) = input.next().await {
            let eth_watch_req = eth_watch_req.clone();
            handle.spawn(async move {
                let resp = verify_eth_signature(&req, eth_watch_req)
                    .await
                    .and_then(|_| verify_tx_correctness(&req.tx));

                req.response.send(resp).unwrap_or_default();
            });
        }
    }

    std::thread::Builder::new()
        .name("verify 2fa signature".to_string())
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
        .expect("failed to start 2fa verifier");
}
