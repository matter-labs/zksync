//! Module capable of checking the onchain operations, such as
//! onchain `ChangePubKey` authorization or EIP1271 signature
//! verification.

use web3::{
    contract::{Contract, Options},
    types::{Address, H160},
    Transport, Web3,
};
use zksync_contracts::{eip1271_contract, zksync_contract};
use zksync_types::{
    tx::EIP1271Signature,
    {Nonce, PubKeyHash},
};

/// isValidSignature return value according to EIP1271 standard
/// bytes4(keccak256("isValidSignature(bytes32,bytes)")
pub const EIP1271_SUCCESS_RETURN_VALUE: [u8; 4] = [0x16, 0x26, 0xba, 0x7e];

#[derive(Clone)]
pub struct EthereumChecker<T: Transport> {
    web3: Web3<T>,
    zksync_contract: (ethabi::Contract, Contract<T>),
}

impl<T: Transport> EthereumChecker<T> {
    pub fn new(web3: Web3<T>, zksync_contract_addr: H160) -> Self {
        let zksync_contract = {
            (
                zksync_contract(),
                Contract::new(web3.eth(), zksync_contract_addr, zksync_contract()),
            )
        };

        Self {
            zksync_contract,
            web3,
        }
    }

    fn get_eip1271_contract(&self, address: Address) -> Contract<T> {
        Contract::new(self.web3.eth(), address, eip1271_contract())
    }

    /// Transforms the message into an array expected by EIP-1271 standard.
    fn get_sign_message(message: &[u8]) -> [u8; 32] {
        // sign_message = keccak256("\x19Ethereum Signed Message:\n{msg_len}" + message))
        let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
        let mut bytes = Vec::with_capacity(prefix.len() + message.len());
        bytes.extend_from_slice(prefix.as_bytes());
        bytes.extend_from_slice(message);
        tiny_keccak::keccak256(&bytes)
    }

    pub async fn is_eip1271_signature_correct(
        &self,
        address: Address,
        message: &[u8],
        signature: EIP1271Signature,
    ) -> Result<bool, anyhow::Error> {
        let sign_message = Self::get_sign_message(message);

        let call_result = self
            .get_eip1271_contract(address)
            .query(
                "isValidSignature",
                (sign_message, signature.0),
                Some(address),
                Options::default(),
                None,
            )
            .await;

        let received: [u8; 4] = match call_result {
            Ok(val) => val,
            Err(error) => {
                // One error of this kind will mean that user provided incorrect signature.
                // Many errors will likely mean that something is wrong with our implementation.
                log::warn!("EIP1271 signature check failed: {:#?}", error);
                return Ok(false);
            }
        };

        Ok(received == EIP1271_SUCCESS_RETURN_VALUE)
    }

    pub async fn is_new_pubkey_hash_authorized(
        &self,
        address: Address,
        nonce: Nonce,
        pub_key_hash: &PubKeyHash,
    ) -> Result<bool, anyhow::Error> {
        let auth_fact: Vec<u8> = self
            .zksync_contract
            .1
            .query(
                "authFacts",
                (address, u64::from(nonce)),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| anyhow::format_err!("Failed to query contract authFacts: {}", e))?;
        Ok(auth_fact.as_slice() == tiny_keccak::keccak256(&pub_key_hash.data[..]))
    }
}

#[cfg(test)]
mod tests {
    use super::EthereumChecker;
    use std::str::FromStr;
    use zksync_config::test_config::TestConfig;
    use zksync_types::{
        tx::{EIP1271Signature, PackedEthSignature},
        Address,
    };

    #[tokio::test]
    async fn test_eip1271() {
        let config = TestConfig::load();
        let message = "hello-world";

        let manual_signature =
            PackedEthSignature::sign(&config.eip1271.owner_private_key, message.as_bytes())
                .unwrap();
        let signature = EIP1271Signature(manual_signature.serialize_packed().to_vec());

        let transport = web3::transports::Http::new(&config.eth.web3_url).unwrap();
        let web3 = web3::Web3::new(transport);

        let eth_checker = EthereumChecker::new(web3, Default::default());
        let result = eth_checker
            .is_eip1271_signature_correct(
                config.eip1271.contract_address,
                message.as_bytes(),
                signature,
            )
            .await
            .expect("Check failed");

        assert_eq!(result, true, "Signature is incorrect");
    }

    /// This test checks that the actual signature data taken from
    /// mainnet / Argent smart wallet is valid in our codebase.
    #[test]
    fn actual_data_check() {
        // Signature data obtained from the actual EIP-1271 signature made via Argent.
        const SIG_DATA: &str = "ebbb656a980792465a98aff29ecfd43f3cd94b4ef9490535565d5242fb55208c67c3006cc166ef66b1064282ed26ee0bc54d6b2c28cb779a642b8e9e2aad5e361c";
        // Smart wallet contract address.
        // const ACCOUNT_ADDR: &str = "730094414795264fD9579c4aC816Cb1C0F4A545E";
        // Actual account owner address.
        const ACCOUNT_OWNER_ADDR: &str = "b6c3dd5a0e5f10f82f2a07fad0aef8cd5ce8c670";
        // Message that was used for signing.
        const MESSAGE: &str = "hello-world";

        let signature_data = hex::decode(SIG_DATA).unwrap();

        let modified_message =
            EthereumChecker::<web3::transports::Http>::get_sign_message(MESSAGE.as_bytes());
        // Here we use `web3::signing` module for purpose to not interfer with our own recovering implementation.
        // Otherwise it's possible that signing / recovering will overlap with the same error.
        let restored_address = web3::signing::recover(
            &modified_message,
            &signature_data[..64],
            (signature_data[64] - 27) as i32,
        )
        .expect("Cannot recover");

        let expected_address = Address::from_str(ACCOUNT_OWNER_ADDR).unwrap();
        assert_eq!(
            restored_address, expected_address,
            "Restored address is incorrect"
        );
    }
}
