use crate::SignerError;
use models::tx::PackedEthSignature;
use models::tx::TxEthSignature;
use models::Address;
use models::H256;

#[derive(Clone)]
pub struct PrivateKeySigner {
    private_key: H256,
}

impl PrivateKeySigner {
    pub fn new(private_key: H256) -> Self {
        Self { private_key }
    }

    pub fn sign(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        let pack = PackedEthSignature::sign(&self.private_key, message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(TxEthSignature::EthereumSignature(pack))
    }

    pub fn address(&self) -> Address {
        Address::default()
        // todo1 magic code
    }
}
