// Signer. TODO: describe what's here.
// NOTE: From: https://github.com/matter-labs/zksync-dev/blob/dev/core/testkit/src/zksync_account.rs
// Built-in imports
use std::{fmt, sync::Mutex};
// External uses
use num::BigUint;
use web3::types::H256;
// Workspace uses
use franklin_crypto::alt_babyjubjub::fs::FsRepr;
use franklin_crypto::bellman::{pairing::ff::PrimeField, PrimeFieldRepr};
use models::node::tx::{ChangePubKey, PackedEthSignature, TxSignature};
use models::node::{
    AccountId, Address, Close, Fs, Nonce, PrivateKey, PubKeyHash, TokenId, Transfer, Withdraw,
};
use sha2::{Digest, Sha256};
pub struct Signer {
    pub private_key: PrivateKey,
    pub pubkey_hash: PubKeyHash,
    pub address: Address,
    pub eth_private_key: H256,
    account_id: Mutex<Option<AccountId>>, // NOTE: this field is never used. Should I remove it?
    nonce: Mutex<Nonce>,
}

impl fmt::Debug for Signer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // It is OK to disclose the private key contents for a testkit account.
        let mut pk_contents = Vec::new();
        self.private_key
            .write(&mut pk_contents)
            .expect("Failed writing the private key contents");
        f.debug_struct("Signer")
            .field("private_key", &pk_contents)
            .field("pubkey_hash", &self.pubkey_hash)
            .field("address", &self.address)
            .field("eth_private_key", &self.eth_private_key)
            .field("nonce", &self.nonce)
            .finish()
    }
}

impl Signer {
    pub fn new(
        private_key: PrivateKey,
        nonce: Nonce,
        address: Address,
        eth_private_key: H256,
    ) -> Self {
        let pubkey_hash = PubKeyHash::from_privkey(&private_key);
        assert_eq!(
            address,
            PackedEthSignature::address_from_private_key(&eth_private_key)
                .expect("private key is incorrect"),
            "address should correspond to private key"
        );
        Self {
            private_key,
            pubkey_hash,
            address,
            eth_private_key: eth_private_key,
            account_id: Mutex::new(None),
            nonce: Mutex::new(nonce),
        }
    }

    pub fn new_from_seed(
        seed: &[u8],
        nonce: Nonce,
        address: Address,
        eth_private_key: H256,
    ) -> Signer {
        let pk = Self::private_key_from_seed(seed);
        let pk_array: &[u8] = &pk;
        let private_key = PrivateKey::read(pk_array).unwrap(); // TODO: refactor unwrap
        Self::new(private_key, nonce, address, eth_private_key)
    }

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
    pub fn pubkey_hash(&self) -> &PubKeyHash {
        &self.pubkey_hash
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sign_transfer(
        &self,
        token_id: TokenId,
        token_symbol: &str,
        amount: BigUint,
        fee: BigUint,
        to: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> (Transfer, PackedEthSignature) {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let transfer = Transfer::new_signed(
            self.account_id
                .lock()
                .unwrap()
                .expect("can't sign tx withoud account id"),
            self.address,
            *to,
            token_id,
            amount,
            fee,
            nonce.unwrap_or_else(|| *stored_nonce),
            &self.private_key,
        )
        .expect("Failed to sign transfer");

        if increment_nonce {
            *stored_nonce += 1;
        }

        let eth_signature = PackedEthSignature::sign(
            &self.eth_private_key,
            transfer
                .get_ethereum_sign_message(token_symbol, 18)
                .as_bytes(),
        )
        .expect("Signing the transfer unexpectedly failed");
        (transfer, eth_signature)
    }

    // NOTE: the rest of these aren't in the spec, but I kept them in. Possibly change their public scope?
    pub fn nonce(&self) -> Nonce {
        let n = self.nonce.lock().unwrap();
        *n
    }

    pub fn set_nonce(&self, new_nonce: Nonce) {
        *self.nonce.lock().unwrap() = new_nonce;
    }

    pub fn set_account_id(&self, account_id: Option<AccountId>) {
        *self.account_id.lock().unwrap() = account_id;
    }

    pub fn get_account_id(&self) -> Option<AccountId> {
        *self.account_id.lock().unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sign_withdraw(
        &self,
        token_id: TokenId,
        token_symbol: &str,
        amount: BigUint,
        fee: BigUint,
        eth_address: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> (Withdraw, PackedEthSignature) {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let withdraw = Withdraw::new_signed(
            self.account_id
                .lock()
                .unwrap()
                .expect("can't sign tx withoud account id"),
            self.address,
            *eth_address,
            token_id,
            amount,
            fee,
            nonce.unwrap_or_else(|| *stored_nonce),
            &self.private_key,
        )
        .expect("Failed to sign withdraw");

        if increment_nonce {
            *stored_nonce += 1;
        }

        let eth_signature = PackedEthSignature::sign(
            &self.eth_private_key,
            withdraw
                .get_ethereum_sign_message(token_symbol, 18)
                .as_bytes(),
        )
        .expect("Signing the withdraw unexpectedly failed");
        (withdraw, eth_signature)
    }

    pub fn sign_close(&self, nonce: Option<Nonce>, increment_nonce: bool) -> Close {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let mut close = Close {
            account: self.address,
            nonce: nonce.unwrap_or_else(|| *stored_nonce),
            signature: TxSignature::default(),
        };
        close.signature = TxSignature::sign_musig(&self.private_key, &close.get_bytes());

        if increment_nonce {
            *stored_nonce += 1;
        }
        close
    }

    pub fn create_change_pubkey_tx(
        &self,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        auth_onchain: bool,
    ) -> ChangePubKey {
        let account_id = self
            .account_id
            .lock()
            .unwrap()
            .expect("can't sign tx withoud account id");
        let mut stored_nonce = self.nonce.lock().unwrap();
        let nonce = nonce.unwrap_or_else(|| *stored_nonce);
        let eth_signature = if auth_onchain {
            None
        } else {
            let sign_bytes =
                ChangePubKey::get_eth_signed_data(account_id, nonce, &self.pubkey_hash)
                    .expect("Failed to construct change pubkey signed message.");
            let eth_signature = PackedEthSignature::sign(&self.eth_private_key, &sign_bytes)
                .expect("Signature should succeed");
            Some(eth_signature)
        };
        let change_pubkey = ChangePubKey {
            account_id,
            account: self.address,
            new_pk_hash: self.pubkey_hash.clone(),
            nonce,
            eth_signature,
        };

        if !auth_onchain {
            assert!(
                change_pubkey.verify_eth_signature() == Some(self.address),
                "eth signature is incorrect"
            );
        }

        if increment_nonce {
            *stored_nonce += 1;
        }

        change_pubkey
    }
}
