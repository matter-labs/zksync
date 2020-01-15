use bigdecimal::BigDecimal;
use franklin_crypto::jubjub::FixedGenerators;
use models::node::tx::{PackedPublicKey, PackedSignature, TxSignature};
use models::node::{
    priv_key_from_fs, AccountAddress, AccountId, FullExit, Nonce, PrivateKey, PublicKey, TokenId,
    Transfer, Withdraw,
};
use models::params::{JUBJUB_PARAMS, SIGNATURE_R_BIT_WIDTH_PADDED, SIGNATURE_S_BIT_WIDTH_PADDED};
use rand::{thread_rng, Rng};
use std::cell::RefCell;
use std::convert::TryInto;
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

    pub fn sign_full_exit(
        &self,
        account_id: AccountId,
        eth_address: Address,
        token: TokenId,
    ) -> FullExit {
        let pub_key = PackedPublicKey(PublicKey::from_private(
            &self.private_key,
            FixedGenerators::SpendingKeyGenerator,
            &JUBJUB_PARAMS,
        ));

        let mut full_exit = FullExit {
            account_id,
            packed_pubkey: Box::new(
                pub_key
                    .serialize_packed()
                    .expect("pk serialize")
                    .as_slice()
                    .try_into()
                    .unwrap(),
            ),
            eth_address,
            token,
            nonce: *self.nonce.borrow(),
            signature_r: Box::new([0u8; 32]),
            signature_s: Box::new([0u8; 32]),
        };

        let signature_bytes =
            TxSignature::sign_musig_pedersen(&self.private_key, &full_exit.get_bytes())
                .signature
                .serialize_packed()
                .expect("signature serialize");
        full_exit.signature_r = Box::new(signature_bytes[0..32].try_into().unwrap());
        full_exit.signature_s = Box::new(signature_bytes[32..].try_into().unwrap());

        *self.nonce.borrow_mut() += 1;
        full_exit
    }
}
