use crate::raw_ethereum_tx::RawTransaction;
use crate::{EthereumSigner, SignerError};

use parity_crypto::publickey::sign;

use zksync_types::eip712_signature::{EIP712TypedStructure, Eip712Domain};
use zksync_types::tx::{PackedEthSignature, TxEthSignature};
use zksync_types::{Address, H256};

#[derive(Clone)]
pub struct PrivateKeySigner {
    private_key: H256,
}

impl std::fmt::Debug for PrivateKeySigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrivateKeySigner")
    }
}

impl PrivateKeySigner {
    pub fn new(private_key: H256) -> Self {
        Self { private_key }
    }
}

#[async_trait::async_trait]
impl EthereumSigner for PrivateKeySigner {
    /// Get Ethereum address that matches the private key.
    async fn get_address(&self) -> Result<Address, SignerError> {
        PackedEthSignature::address_from_private_key(&self.private_key)
            .map_err(|_| SignerError::DefineAddress)
    }

    /// The sign method calculates an Ethereum specific signature with:
    /// sign(keccak256("\x19Ethereum Signed Message:\n" + len(message) + message))).
    async fn sign_message(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        let pack = PackedEthSignature::sign(&self.private_key, message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(TxEthSignature::EthereumSignature(pack))
    }

    /// Signs and returns the RLP-encoded transaction.
    async fn sign_transaction(&self, raw_tx: RawTransaction) -> Result<Vec<u8>, SignerError> {
        let sig = sign(&self.private_key.into(), &raw_tx.hash().into())
            .map_err(|_| SignerError::NoSigningKey)?;
        Ok(raw_tx.rlp_encode_tx(sig))
    }

    async fn sign_typed_data<S: EIP712TypedStructure + Sync>(
        &self,
        eip712_domain: &Eip712Domain,
        typed_struct: &S,
    ) -> Result<PackedEthSignature, SignerError> {
        PackedEthSignature::sign_typed_data(&self.private_key, eip712_domain, typed_struct)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))
    }
}

#[cfg(test)]
mod test {
    use super::PrivateKeySigner;
    use super::RawTransaction;
    use crate::EthereumSigner;
    use zksync_types::tx::{eip712_signature::Eip712Domain, ChangePubKey, PackedEthSignature};
    use zksync_types::{AccountId, ChainId, Nonce, PubKeyHash, H160, H256, U256};

    #[tokio::test]
    async fn test_generating_signature() {
        let private_key = H256::from([5; 32]);
        let signer = PrivateKeySigner::new(private_key);
        let raw_transaction = RawTransaction {
            chain_id: 1,
            nonce: U256::from(1),
            to: Some(H160::zero()),
            value: U256::from(10),
            gas_price: U256::from(1),
            gas: U256::from(2),
            data: vec![1, 2, 3],
        };
        let signature = signer
            .sign_transaction(raw_transaction.clone())
            .await
            .unwrap();
        assert_ne!(signature.len(), 1);
        // precalculated signature with right algorithm implementation
        let precalculated_signature: Vec<u8> = vec![
            248, 96, 1, 1, 2, 148, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10,
            131, 1, 2, 3, 37, 160, 152, 202, 15, 174, 50, 167, 190, 239, 206, 183, 109, 215, 135,
            60, 43, 71, 11, 74, 252, 97, 83, 86, 66, 249, 237, 111, 118, 121, 105, 214, 130, 249,
            160, 106, 110, 143, 138, 113, 12, 177, 239, 121, 188, 247, 21, 236, 236, 163, 254, 28,
            48, 250, 5, 20, 234, 54, 58, 162, 103, 252, 20, 243, 121, 7, 19,
        ];
        assert_eq!(signature, precalculated_signature);
    }

    #[tokio::test]
    async fn change_pub_key_signature() {
        let pubkey_hash =
            PubKeyHash::from_hex("sync:fefefefefefefefefefefefefefefefefefefefe").unwrap();
        let account_id = AccountId(0xdeadba);
        let nonce = Nonce(0x11223344);

        let pk = hex::decode("d26b03c9b31e2b4d09b4761ea86b3e0a32dd23b3886c7111280d73f0c34c1841")
            .unwrap();
        let pk_signer = PrivateKeySigner::new(H256::from_slice(&pk));

        let chain_id = ChainId(31337);
        let domain = Eip712Domain::new(chain_id);
        let change_pub_key = ChangePubKey::new(
            account_id,
            Default::default(),
            pubkey_hash,
            Default::default(),
            Default::default(),
            nonce,
            Default::default(),
            None,
            None,
            Some(chain_id),
        );

        let signature = pk_signer
            .sign_typed_data(&domain, &change_pub_key)
            .await
            .unwrap();
        let expected_signature = hex::decode("ea37325672e61e170663ea4fc5db2831c6f8251c4d9e24bb247da0cc50aedd5c615f7a6b18d90e3486fc767b54785467938912301a9b3e7ba474e27cdbb56c471c").unwrap();
        assert_eq!(signature.serialize_packed().to_vec(), expected_signature);
        let hash = PackedEthSignature::typed_data_to_signed_bytes(&domain, &change_pub_key);
        let address = signature.signature_recover_signer_from_hash(hash).unwrap();
        assert_eq!(
            address,
            H160::from_slice(&hex::decode("c85065ab91b00cc3d121f29ac7f5335f3a902c41").unwrap())
        )
    }

    #[tokio::test]
    async fn change_pub_key_signature_2() {
        let pubkey_hash =
            PubKeyHash::from_hex("sync:09aa618dcd099a62dbdfa81b4c1d08aa8e4bd5d2").unwrap();
        let account_id = AccountId(4);
        let nonce = Nonce(0);

        let chain_id = ChainId(9);
        let change_pub_key = ChangePubKey::new(
            account_id,
            H160::from_slice(&hex::decode("f58281a60234ec8e9b1efe30209e6786a12dfe91").unwrap()),
            pubkey_hash,
            Default::default(),
            Default::default(),
            nonce,
            Default::default(),
            None,
            None,
            Some(chain_id),
        );

        let signature = PackedEthSignature::deserialize_packed(&vec![
            119, 9, 17, 255, 181, 230, 46, 68, 33, 132, 97, 20, 114, 80, 116, 36, 11, 103, 205, 4,
            119, 126, 149, 136, 40, 0, 125, 94, 144, 110, 169, 204, 93, 64, 113, 69, 184, 5, 12,
            234, 226, 126, 72, 215, 177, 100, 245, 141, 57, 198, 200, 95, 221, 251, 25, 193, 243,
            66, 49, 34, 63, 105, 47, 26, 28,
        ])
        .unwrap();

        let domain = Eip712Domain::new(chain_id);
        let data = PackedEthSignature::typed_data_to_signed_bytes(&domain, &change_pub_key);
        let recovered_address = signature.signature_recover_signer_from_hash(data).unwrap();

        assert_eq!(recovered_address, change_pub_key.account)
    }
}
