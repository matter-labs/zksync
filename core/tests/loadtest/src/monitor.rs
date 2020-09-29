// Built-in import
// External uses
// Workspace uses
use models::{tx::PackedEthSignature, tx::TxHash, FranklinTx};
use zksync::{error::ClientError, Provider};
// Local uses

#[derive(Debug, Clone)]
pub struct Monitor {
    pub provider: Provider,
}

impl Monitor {
    pub async fn send_tx(
        &self,
        tx: FranklinTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<TxHash, ClientError> {
        self.provider.send_tx(tx, eth_signature).await
    }
}
