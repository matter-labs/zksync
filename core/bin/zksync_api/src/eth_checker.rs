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

    pub async fn is_eip1271_signature_correct(
        &self,
        address: Address,
        message: Vec<u8>,
        signature: EIP1271Signature,
    ) -> Result<bool, anyhow::Error> {
        let hash = tiny_keccak::keccak256(&message);

        let received: [u8; 4] = self
            .get_eip1271_contract(address)
            .query(
                "isValidSignature",
                (hash, signature.0),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| anyhow::format_err!("Failed to query contract isValidSignature: {}", e))?;

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
