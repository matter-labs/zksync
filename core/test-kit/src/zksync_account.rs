use bigdecimal::BigDecimal;
use models::node::tx::TxSignature;
use models::node::{
    priv_key_from_fs, AccountAddress, Nonce, PrivateKey, PublicKey, TokenId, Transfer, Withdraw,
};
use rand::{thread_rng, Rng};
use std::cell::RefCell;
use web3::types::Address;

pub struct ZksyncAccount {
    pub private_key: PrivateKey,
    pub address: AccountAddress,
    nonce: RefCell<Nonce>,
}

impl ZksyncAccount {
    pub fn rand() -> Self {
        let mut rng = &mut thread_rng();

        let pk = priv_key_from_fs(rng.gen());
        Self::new(pk, 0)
    }

    pub fn new(private_key: PrivateKey, nonce: Nonce) -> Self {
        Self {
            address: AccountAddress::from_privkey(&private_key),
            private_key,
            nonce: RefCell::new(nonce),
        }
    }
    pub fn sign_transfer(
        &self,
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
        to: &AccountAddress,
    ) -> Transfer {
        let mut transfer = Transfer {
            from: self.address.clone(),
            to: to.clone(),
            token: token_id,
            amount,
            fee,
            nonce: *self.nonce.borrow(),
            signature: TxSignature::default(),
        };
        transfer.signature =
            TxSignature::sign_musig_pedersen(&self.private_key, &transfer.get_bytes());

        *self.nonce.borrow_mut() += 1;
        transfer
    }

    pub fn sign_withdraw(
        &self,
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
        eth_address: &Address,
    ) -> Withdraw {
        let mut withdraw = Withdraw {
            account: self.address.clone(),
            eth_address: eth_address.clone(),
            token: token_id,
            amount,
            fee,
            nonce: *self.nonce.borrow(),
            signature: TxSignature::default(),
        };
        withdraw.signature =
            TxSignature::sign_musig_pedersen(&self.private_key, &withdraw.get_bytes());

        *self.nonce.borrow_mut() += 1;
        withdraw
    }
}
