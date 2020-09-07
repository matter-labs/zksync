// Wallet: TODO describe what's here
use crate::{provider::Provider, signer::Signer};
use anyhow;
use franklin_crypto::alt_babyjubjub::fs::FsRepr; // is this already in the models crate somewhere?

use bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use models::node::{tx::TxHash, Address, Fs};
use sha2::{Digest, Sha256};

#[derive(Debug)]
struct Wallet {
    pub provider: Provider,
    pub signer: Signer,
    pub address: Address, // note2self: from web3: pub type Address = H160;
                          /* other stuff? */
}

// NOTE: This was was not strictly copy paste from elsewhere, deserves higher scrutiny
impl Wallet {
    pub fn new(provider: Provider, signer: Signer, address: Address) -> Self {
        Wallet {
            provider,
            signer,
            address,
        }
    }
    // from: https://github.com/matter-labs/zks-crypto/blob/master/zks-crypto-c/src/utils.rs#L113
    pub fn private_key_from_seed(seed: &[u8]) -> Vec<u8> {
        let sha256_bytes = |input: &[u8]| -> Vec<u8> {
            let mut hasher = Sha256::new();
            hasher.input(input);
            hasher.result().to_vec()
        };

        let mut effective_seed = sha256_bytes(seed);

        loop {
            let raw_priv_key = sha256_bytes(&effective_seed);
            let mut fs_repr = FsRepr::default();
            fs_repr
                .read_be(&raw_priv_key[..])
                .expect("failed to read raw_priv_key");
            if Fs::from_repr(fs_repr).is_ok() {
                return raw_priv_key;
            } else {
                effective_seed = raw_priv_key;
            }
        }
    }
    pub fn address_from_private_key(private_key: Vec<u8>) -> Address {
        unimplemented!();
    }

    //Derive address from eth_pk, derive signer from signature of login message using eth_pk as a seed.
    pub fn new_private_key_from_seed(seed: &[u8], provider: Provider) -> Self {
        unimplemented!();
        //let privkey = Self::private_key_from_seed(seed);
    }

    // Sign transaction with a signer and sumbit it using provider.
    // add args
    async fn transfer(&self) -> Result<TxHash, anyhow::Error> {
        unimplemented!();
    }
}
