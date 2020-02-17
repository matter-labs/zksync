use bigdecimal::BigDecimal;
use crypto_exports::rand::{thread_rng, Rng};
use models::node::tx::{ChangePubKey, PackedEthSignature, TxSignature};
use models::node::{
    priv_key_from_fs, Address, Nonce, PrivateKey, PubKeyHash, TokenId, Transfer, Withdraw,
};
use std::cell::RefCell;
use web3::types::H256;

/// Structure used to sign ZKSync transactions, keeps tracks of its nonce internally
pub struct ZksyncAccount {
    pub private_key: PrivateKey,
    pub pubkey_hash: PubKeyHash,
    pub address: Address,
    pub eth_private_key: H256,
    nonce: RefCell<Nonce>,
}

impl ZksyncAccount {
    /// Note: probably not secure, use for testing.
    pub fn rand() -> Self {
        let rng = &mut thread_rng();

        let pk = priv_key_from_fs(rng.gen());
        let (eth_pk, eth_address) = {
            let eth_pk = rng.gen::<[u8; 32]>().into();
            let eth_address;
            loop {
                if let Ok(address) = PackedEthSignature::address_from_private_key(&eth_pk) {
                    eth_address = address;
                    break;
                }
            }
            (eth_pk, eth_address)
        };
        Self::new(pk, 0, eth_address, eth_pk)
    }

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
            address,
            private_key,
            pubkey_hash,
            eth_private_key,
            nonce: RefCell::new(nonce),
        }
    }

    pub fn nonce(&self) -> Nonce {
        *self.nonce.borrow()
    }

    pub fn sign_transfer(
        &self,
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
        to: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> Transfer {
        let mut transfer = Transfer {
            from: self.address,
            to: *to,
            token: token_id,
            amount,
            fee,
            nonce: nonce.unwrap_or_else(|| *self.nonce.borrow()),
            signature: TxSignature::default(),
        };
        transfer.signature =
            TxSignature::sign_musig_sha256(&self.private_key, &transfer.get_bytes());

        if increment_nonce {
            *self.nonce.borrow_mut() += 1;
        }
        transfer
    }

    pub fn sign_withdraw(
        &self,
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
        eth_address: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> Withdraw {
        let mut withdraw = Withdraw {
            from: self.address,
            to: *eth_address,
            token: token_id,
            amount,
            fee,
            nonce: nonce.unwrap_or_else(|| *self.nonce.borrow()),
            signature: TxSignature::default(),
        };
        withdraw.signature =
            TxSignature::sign_musig_sha256(&self.private_key, &withdraw.get_bytes());

        if increment_nonce {
            *self.nonce.borrow_mut() += 1;
        }
        withdraw
    }

    pub fn create_change_pubkey_tx(
        &self,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        auth_onchain: bool,
    ) -> ChangePubKey {
        let nonce = nonce.unwrap_or_else(|| *self.nonce.borrow());
        let eth_signature = if auth_onchain {
            None
        } else {
            let sign_bytes = ChangePubKey::get_eth_signed_data(nonce, &self.pubkey_hash);
            let eth_signature = PackedEthSignature::sign(&self.eth_private_key, &sign_bytes)
                .expect("Signature should succeed");
            Some(eth_signature)
        };
        let change_pubkey = ChangePubKey {
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
            *self.nonce.borrow_mut() += 1;
        }

        change_pubkey
    }
}
