use num::BigUint;
use zksync::utils::{closest_packable_token_amount, private_key_from_seed};
use zksync_types::{
    tx::{PackedEthSignature, TxSignature},
    TokenId, ZkSyncTx, H256,
};

use crate::command::IncorrectnessModifier;

pub trait Corrupted: Sized {
    fn bad_zksync_signature(self) -> Self;
    fn bad_eth_signature(self) -> Self;
    fn nonexistent_token(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    fn not_packable_amount(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    fn not_packable_fee(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    fn too_big_amount(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    fn zero_fee(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;

    fn resign(&mut self, eth_pk: H256, token_symbol: &str, decimals: u8);

    fn apply_modifier(
        self,
        modifier: IncorrectnessModifier,
        eth_pk: H256,
        token_symbol: &str,
        decimals: u8,
    ) -> Self {
        match modifier {
            IncorrectnessModifier::None => self,
            IncorrectnessModifier::IncorrectEthSignature => self.bad_eth_signature(),
            IncorrectnessModifier::IncorrectZkSyncSignature => self.bad_zksync_signature(),
            IncorrectnessModifier::NonExistentToken => {
                self.nonexistent_token(eth_pk, token_symbol, decimals)
            }
            IncorrectnessModifier::NotPackableAmount => {
                self.not_packable_amount(eth_pk, token_symbol, decimals)
            }
            IncorrectnessModifier::NotPackableFeeAmount => {
                self.not_packable_fee(eth_pk, token_symbol, decimals)
            }
            IncorrectnessModifier::TooBigAmount => {
                self.too_big_amount(eth_pk, token_symbol, decimals)
            }
            IncorrectnessModifier::ZeroFee => self.zero_fee(eth_pk, token_symbol, decimals),
        }
    }
}

impl Corrupted for (ZkSyncTx, Option<PackedEthSignature>) {
    fn resign(&mut self, eth_pk: H256, token_symbol: &str, decimals: u8) {
        let zksync_pk = private_key_from_seed(eth_pk.as_bytes()).unwrap();

        let eth_message = match &mut self.0 {
            ZkSyncTx::ChangePubKey(tx) => {
                tx.signature = TxSignature::sign_musig(&zksync_pk, &tx.get_bytes());
                tx.get_eth_signed_data().unwrap()
            }
            ZkSyncTx::ForcedExit(tx) => {
                tx.signature = TxSignature::sign_musig(&zksync_pk, &tx.get_bytes());
                tx.get_ethereum_sign_message(token_symbol, decimals)
                    .as_bytes()
                    .to_vec()
            }
            ZkSyncTx::Transfer(tx) => {
                tx.signature = TxSignature::sign_musig(&zksync_pk, &tx.get_bytes());
                tx.get_ethereum_sign_message(token_symbol, decimals)
                    .as_bytes()
                    .to_vec()
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.signature = TxSignature::sign_musig(&zksync_pk, &tx.get_bytes());
                tx.get_ethereum_sign_message(token_symbol, decimals)
                    .as_bytes()
                    .to_vec()
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
        };

        if let Some(eth_sig) = &mut self.1 {
            *eth_sig = PackedEthSignature::sign(&eth_pk, &eth_message)
                .expect("Signing the transfer unexpectedly failed")
        }
    }

    fn bad_eth_signature(self) -> Self {
        let private_key = H256::random();
        let message = b"bad message";
        (
            self.0,
            self.1
                .and_then(|_| PackedEthSignature::sign(&private_key, message).ok()),
        )
    }

    fn bad_zksync_signature(mut self) -> Self {
        let bad_signature = TxSignature::default();
        match &mut self.0 {
            ZkSyncTx::ChangePubKey(tx) => {
                tx.signature = bad_signature;
            }
            ZkSyncTx::ForcedExit(tx) => {
                tx.signature = bad_signature;
            }
            ZkSyncTx::Transfer(tx) => {
                tx.signature = bad_signature;
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.signature = bad_signature;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
        }
        self
    }

    fn nonexistent_token(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        let bad_token = TokenId(199u16); // Assuming that on the stand there will be much less tokens.
        match &mut self.0 {
            ZkSyncTx::ChangePubKey(tx) => {
                tx.fee_token = bad_token;
            }
            ZkSyncTx::ForcedExit(tx) => {
                tx.token = bad_token;
            }
            ZkSyncTx::Transfer(tx) => {
                tx.token = bad_token;
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.token = bad_token;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }

    fn not_packable_amount(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        let bad_amount = BigUint::from(10u64.pow(10)) + BigUint::from(1u64);
        match &mut self.0 {
            ZkSyncTx::ChangePubKey(_tx) => unreachable!("CPK doesn't have amount"),
            ZkSyncTx::ForcedExit(_tx) => unreachable!("ForcedExit doesn't have amount"),
            ZkSyncTx::Transfer(tx) => {
                tx.amount = bad_amount;
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.amount = bad_amount;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }

    fn not_packable_fee(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        let bad_fee = BigUint::from(10u64.pow(10)) + BigUint::from(1u64);
        match &mut self.0 {
            ZkSyncTx::ChangePubKey(tx) => {
                tx.fee = bad_fee;
            }
            ZkSyncTx::ForcedExit(tx) => {
                tx.fee = bad_fee;
            }
            ZkSyncTx::Transfer(tx) => {
                tx.fee = bad_fee;
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.fee = bad_fee;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }

    fn too_big_amount(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        // We want to fail tx because of the amount, not because of packability.
        let big_amount = closest_packable_token_amount(&BigUint::from(u64::max_value()));
        match &mut self.0 {
            ZkSyncTx::ChangePubKey(_tx) => unreachable!("CPK doesn't have amount"),
            ZkSyncTx::ForcedExit(_tx) => unreachable!("ForcedExit doesn't have amount"),
            ZkSyncTx::Transfer(tx) => {
                tx.amount = big_amount;
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.amount = big_amount;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }

    fn zero_fee(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        let zero_fee = BigUint::from(0u64);
        match &mut self.0 {
            ZkSyncTx::ChangePubKey(tx) => {
                tx.fee = zero_fee;
            }
            ZkSyncTx::ForcedExit(tx) => {
                tx.fee = zero_fee;
            }
            ZkSyncTx::Transfer(tx) => {
                tx.fee = zero_fee;
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.fee = zero_fee;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }
}
