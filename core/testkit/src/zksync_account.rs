use bigdecimal::BigDecimal;
use franklin_crypto::jubjub::FixedGenerators;
use models::node::tx::{ChangePubKey, PackedEthSignature, PackedPublicKey, TxSignature};
use models::node::{
    priv_key_from_fs, AccountId, Address, FullExit, Nonce, PrivateKey, PubKeyHash, PublicKey,
    TokenId, Transfer, Withdraw,
};
use models::params::JUBJUB_PARAMS;
use rand::{thread_rng, Rng};
use std::cell::RefCell;
use std::convert::TryInto;

/// Structure used to sign ZKSync transactions, keeps tracks of its nonce internally
pub struct ZksyncAccount {
    pub private_key: PrivateKey,
    pub pubkey_hash: PubKeyHash,
    pub address: Address,
    nonce: RefCell<Nonce>,
}

impl ZksyncAccount {
    pub fn rand() -> Self {
        let rng = &mut thread_rng();

        let pk = priv_key_from_fs(rng.gen());
        Self::new(pk, 0, rng.gen::<[u8; 20]>().into())
    }

    pub fn new(private_key: PrivateKey, nonce: Nonce, address: Address) -> Self {
        let pubkey_hash = PubKeyHash::from_privkey(&private_key);
        Self {
            address,
            private_key,
            pubkey_hash,
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
            from: self.address.clone(),
            to: to.clone(),
            token: token_id,
            amount,
            fee,
            nonce: nonce.unwrap_or_else(|| *self.nonce.borrow()),
            signature: TxSignature::default(),
        };
        transfer.signature =
            TxSignature::sign_musig_pedersen(&self.private_key, &transfer.get_bytes());

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
            from: self.address.clone(),
            to: *eth_address,
            token: token_id,
            amount,
            fee,
            nonce: nonce.unwrap_or_else(|| *self.nonce.borrow()),
            signature: TxSignature::default(),
        };
        withdraw.signature =
            TxSignature::sign_musig_pedersen(&self.private_key, &withdraw.get_bytes());

        if increment_nonce {
            *self.nonce.borrow_mut() += 1;
        }
        withdraw
    }

    pub fn create_change_pubkey_tx(
        &self,
        eth_signature: PackedEthSignature,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> ChangePubKey {
        let change_pubkey = ChangePubKey {
            account: self.address,
            new_pk_hash: self.pubkey_hash.clone(),
            nonce: nonce.unwrap_or_else(|| *self.nonce.borrow()),
            eth_signature,
        };

        if increment_nonce {
            *self.nonce.borrow_mut() += 1;
        }

        change_pubkey
    }
}
