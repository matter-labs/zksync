// External uses
use anyhow::ensure;
use tiny_keccak::keccak256;
// Local uses
use super::eth_signature::TxEthSignature;
use crate::{tx::EthSignData, ZkSyncTx};

/// Encapsulates transactions batch signature data. Should only be created via `new()`
/// as long as errors are possible.
#[derive(Debug, Clone)]
pub struct BatchSignData(pub EthSignData);

impl BatchSignData {
    /// Construct the message user is expected to sign for the given batch and pack
    /// it along with signature.
    pub fn new(txs: &[ZkSyncTx], signature: TxEthSignature) -> anyhow::Result<BatchSignData> {
        ensure!(!txs.is_empty(), "Transaction batch cannot be empty");
        // First, check, if `ChangePubKey` is present in the batch. If it is,
        // we expect its signature to be always present and the following message to be signed:
        // change_pub_key_message + keccak256(batch_bytes)
        // This message is supposed to be used for both batch and `ChangePubKey'.
        // However, since `ChangePubKey` only stores Ethereum signature without the message itself,
        // it is also necessary to store batch hash when signing transactions, so we can
        // send it to the smart contract.
        let mut iter = txs.iter().filter_map(|tx| match tx {
            ZkSyncTx::ChangePubKey(tx) => Some(tx),
            _ => None,
        });
        let change_pub_key = iter.next();
        // Multiple `ChangePubKey`s are not allowed in a single batch.
        ensure!(
            iter.next().is_none(),
            "ChangePubKey operation must be unique within a batch"
        );
        // We only prefix the batch message in case `change_pub_key` has Ethereum signature.
        let change_pub_key_message = change_pub_key
            .filter(|tx| tx.eth_signature.is_some())
            .map(|tx| tx.get_eth_signed_data())
            .transpose()?;
        // The hash is already present in `change_pub_key_message`.
        let message = change_pub_key_message.unwrap_or_else(|| {
            keccak256(
                txs.iter()
                    .flat_map(ZkSyncTx::get_bytes)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .to_vec()
        });

        Ok(BatchSignData(EthSignData { signature, message }))
    }
}
