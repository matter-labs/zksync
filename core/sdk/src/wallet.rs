// Wallet: TODO describe what's here
use crate::{provider::Provider, signer::Signer};
use anyhow;
use franklin_crypto::alt_babyjubjub::fs::FsRepr;

use bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use models::node::{tx::TxHash, Address, Fs}; // may want to import priv_key_from_fs
use sha2::{Digest, Sha256};

struct Wallet {
    pub provider: Provider,
    pub signer: Signer,
    pub address: Address,
    /* other stuff */
}

impl Wallet {
    // Derive address from eth_pk, derive signer from signature of login message using eth_pk as a seed.
    // TODO: modify to return Self.
    pub fn new_private_key_from_seed(seed: &[u8], provider: Provider) -> Self {
        unimplemented!();
        // from: https://github.com/matter-labs/zks-crypto/blob/master/zks-crypto-c/src/utils.rs#L113
        /*
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
                //return raw_priv_key;
                                signer = Signer::new(raw_priv_key,)
                                return Wallet{}
            } else {
                effective_seed = raw_priv_key;
            }
        }
                */
    }
    // Sign transaction with a signer and sumbit it using provider.
    async fn transfer(&self) -> Result<TxHash, anyhow::Error> {
        unimplemented!();
    }
}
