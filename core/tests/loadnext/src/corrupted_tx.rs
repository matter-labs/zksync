use num::BigUint;
use zksync::utils::{
    closest_packable_token_amount, is_fee_amount_packable, is_token_amount_packable,
    private_key_from_seed,
};
use zksync_types::{
    tx::{ChangePubKeyECDSAData, ChangePubKeyEthAuthData, PackedEthSignature, TxSignature},
    TokenId, ZkSyncTx, H256,
};

use crate::command::IncorrectnessModifier;

/// Trait that exists solely to extend the signed zkSync transaction interface, providing the ability
/// to modify transaction in a way that will make it invalid.
///
/// Loadtest is expected to simulate the user behavior, and it's not that uncommon of users to send incorrect
/// transactions.
pub trait Corrupted: Sized {
    /// Replaces the zkSync signature with an incorrect one.
    fn bad_zksync_signature(self) -> Self;
    /// Replaces the zkSync 2FA ECDSA signature with an incorrect one.
    /// In case of ChangePubKey, Ethereum signature inside of the transaction will be affected.
    fn bad_eth_signature(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    /// Replaces the transaction token with the non-existing one.
    /// In case of `ChangePubKey` transaction it affects the `fee_token`.
    fn nonexistent_token(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    /// Creates a transaction with a token amount that cannot be packed.
    /// Panics if called with `ChangePubKey` or `ForcedExit.
    fn not_packable_amount(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    /// Creates a transaction with a fee amount that cannot be packed.
    fn not_packable_fee(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    /// Creates a transaction with a token amount that exceeds the wallet balance.
    /// Panics if called with `ChangePubKey` or `ForcedExit.
    fn too_big_amount(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;
    /// Creates a transaction without fee provided.
    fn zero_fee(self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self;

    /// Resigns the transaction after the modification in order to make signatures correct (if applicable).
    fn resign(&mut self, eth_pk: H256, token_symbol: &str, decimals: u8);

    /// Automatically choses one of the methods of this trait based on the provided incorrectness modifiver.
    fn apply_modifier(
        self,
        modifier: IncorrectnessModifier,
        eth_pk: H256,
        token_symbol: &str,
        decimals: u8,
    ) -> Self {
        match modifier {
            IncorrectnessModifier::None => self,
            IncorrectnessModifier::IncorrectEthSignature => {
                self.bad_eth_signature(eth_pk, token_symbol, decimals)
            }
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
            ZkSyncTx::Swap(tx) => {
                tx.signature = TxSignature::sign_musig(&zksync_pk, &tx.get_bytes());
                tx.get_ethereum_sign_message(token_symbol, decimals)
                    .as_bytes()
                    .to_vec()
            }
            ZkSyncTx::MintNFT(tx) => {
                tx.signature = TxSignature::sign_musig(&zksync_pk, &tx.get_bytes());
                tx.get_ethereum_sign_message(token_symbol, decimals)
                    .as_bytes()
                    .to_vec()
            }
            ZkSyncTx::WithdrawNFT(tx) => {
                tx.signature = TxSignature::sign_musig(&zksync_pk, &tx.get_bytes());
                tx.get_ethereum_sign_message(token_symbol, decimals)
                    .as_bytes()
                    .to_vec()
            }
        };

        if let Some(eth_sig) = &mut self.1 {
            *eth_sig = PackedEthSignature::sign(&eth_pk, &eth_message)
                .expect("Signing the transfer unexpectedly failed")
        }
    }

    fn bad_eth_signature(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        let private_key = H256::random();
        let message = b"bad message";
        let bad_signature = PackedEthSignature::sign(&private_key, message).ok();

        if let ZkSyncTx::ChangePubKey(cpk_tx) = &mut self.0 {
            let signature_data = ChangePubKeyECDSAData {
                eth_signature: bad_signature.clone().unwrap(),
                batch_hash: Default::default(),
            };
            cpk_tx.eth_auth_data = Some(ChangePubKeyEthAuthData::ECDSA(signature_data))
        }

        self.resign(eth_pk, token_symbol, decimals);
        let (tx, eth_signature) = self;
        (tx, eth_signature.and(bad_signature))
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
            ZkSyncTx::Swap(tx) => {
                tx.signature = bad_signature;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
            ZkSyncTx::MintNFT(tx) => {
                tx.signature = bad_signature;
            }
            ZkSyncTx::WithdrawNFT(tx) => {
                tx.signature = bad_signature;
            }
        }
        self
    }

    fn nonexistent_token(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        let bad_token = TokenId(199u32); // Assuming that on the stand there will be much less tokens.
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
            ZkSyncTx::Swap(tx) => {
                tx.fee_token = bad_token;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
            ZkSyncTx::MintNFT(tx) => {
                tx.fee_token = bad_token;
            }
            ZkSyncTx::WithdrawNFT(tx) => {
                tx.fee_token = bad_token;
            }
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }

    fn not_packable_amount(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        // We use a decimal-based packing, thus choosing some big power of ten and adding one will
        // predictably result is a non-packable number.
        // Just to be sure that this invariant will be held in the future we have both unit-test and
        // an assertion for that.
        let bad_amount = BigUint::from(10u128.pow(24)) + BigUint::from(1u64);
        assert!(!is_token_amount_packable(&bad_amount));

        match &mut self.0 {
            ZkSyncTx::ChangePubKey(_tx) => unreachable!("CPK doesn't have amount"),
            ZkSyncTx::ForcedExit(_tx) => unreachable!("ForcedExit doesn't have amount"),
            ZkSyncTx::Transfer(tx) => {
                tx.amount = bad_amount;
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.amount = bad_amount;
            }
            ZkSyncTx::Swap(tx) => {
                tx.amounts = (bad_amount.clone(), bad_amount);
            }
            ZkSyncTx::MintNFT(_) => unreachable!("MintNFT doesn't have amount"),
            ZkSyncTx::WithdrawNFT(_) => unreachable!("WithdrawNFT doesn't have amount"),
            ZkSyncTx::Close(_tx) => unreachable!(),
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }

    fn not_packable_fee(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        let bad_fee = BigUint::from(10u64.pow(18)) + BigUint::from(1u64);
        assert!(!is_fee_amount_packable(&bad_fee));

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
            ZkSyncTx::Swap(tx) => {
                tx.fee = bad_fee;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
            ZkSyncTx::MintNFT(tx) => {
                tx.fee = bad_fee;
            }
            ZkSyncTx::WithdrawNFT(tx) => {
                tx.fee = bad_fee;
            }
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }

    fn too_big_amount(mut self, eth_pk: H256, token_symbol: &str, decimals: u8) -> Self {
        // We want to fail tx because of the amount, not because of packability.
        let big_amount = closest_packable_token_amount(&BigUint::from(u128::max_value() >> 32));
        match &mut self.0 {
            ZkSyncTx::ChangePubKey(_tx) => unreachable!("CPK doesn't have amount"),
            ZkSyncTx::ForcedExit(_tx) => unreachable!("ForcedExit doesn't have amount"),
            ZkSyncTx::Transfer(tx) => {
                tx.amount = big_amount;
            }
            ZkSyncTx::Withdraw(tx) => {
                tx.amount = big_amount;
            }
            ZkSyncTx::Swap(tx) => {
                tx.amounts = (big_amount.clone(), big_amount);
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
            ZkSyncTx::MintNFT(_) => unreachable!("MintNFT doesn't have amount"),
            ZkSyncTx::WithdrawNFT(_) => unreachable!("WithdrawNFT doesn't have amount"),
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
            ZkSyncTx::Swap(tx) => {
                tx.fee = zero_fee;
            }
            ZkSyncTx::Close(_tx) => unreachable!(),
            ZkSyncTx::MintNFT(tx) => {
                tx.fee = zero_fee;
            }
            ZkSyncTx::WithdrawNFT(tx) => {
                tx.fee = zero_fee;
            }
        }
        self.resign(eth_pk, token_symbol, decimals);

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_test_account::ZkSyncAccount;
    use zksync_types::{AccountId, Address, Nonce, PubKeyHash, Transfer};

    const AMOUNT: u64 = 100;
    const FEE: u64 = 100;

    fn create_transfer(account: &ZkSyncAccount) -> (ZkSyncTx, Option<PackedEthSignature>) {
        let (transfer, eth_signature) = account.sign_transfer(
            TokenId(0),
            "ETH",
            AMOUNT.into(),
            FEE.into(),
            &Address::repeat_byte(0x7e),
            Some(Nonce(1)),
            false,
            Default::default(),
        );
        let tx = ZkSyncTx::from(transfer);

        (tx, eth_signature)
    }

    fn unwrap_transfer(transfer: ZkSyncTx) -> Transfer {
        if let ZkSyncTx::Transfer(transfer) = transfer {
            *transfer
        } else {
            panic!("Not a transfer")
        }
    }

    fn create_account() -> ZkSyncAccount {
        let mut account = ZkSyncAccount::rand();
        account.set_account_id(Some(AccountId(1)));
        let eth_pk = account.eth_account_data.unwrap_eoa_pk();
        account.private_key = private_key_from_seed(eth_pk.as_bytes()).unwrap();
        account.pubkey_hash = PubKeyHash::from_privkey(&account.private_key);
        account.address = PackedEthSignature::address_from_private_key(&eth_pk).unwrap();
        account
    }

    #[test]
    fn zero_fee() {
        let account = create_account();

        let transfer = create_transfer(&account);

        let (modified_transfer, _eth_signature) =
            transfer.zero_fee(account.eth_account_data.unwrap_eoa_pk(), "ETH", 18);

        assert_eq!(unwrap_transfer(modified_transfer).fee, 0u64.into());
    }

    #[test]
    fn too_big_amount() {
        let account = create_account();

        let transfer = create_transfer(&account);

        let (modified_transfer, _eth_signature) =
            transfer.too_big_amount(account.eth_account_data.unwrap_eoa_pk(), "ETH", 18);

        assert!(unwrap_transfer(modified_transfer).amount > AMOUNT.into());
    }

    #[test]
    fn not_packable_amount() {
        let account = create_account();

        let transfer = create_transfer(&account);

        let (modified_transfer, _eth_signature) =
            transfer.not_packable_amount(account.eth_account_data.unwrap_eoa_pk(), "ETH", 18);

        assert_eq!(
            is_token_amount_packable(&unwrap_transfer(modified_transfer).amount),
            false
        );
    }

    #[test]
    fn not_packable_fee() {
        let account = create_account();

        let transfer = create_transfer(&account);

        let (modified_transfer, _eth_signature) =
            transfer.not_packable_fee(account.eth_account_data.unwrap_eoa_pk(), "ETH", 18);

        assert_eq!(
            is_fee_amount_packable(&unwrap_transfer(modified_transfer).fee),
            false
        );
    }

    #[test]
    fn nonexistent_token() {
        let account = create_account();

        let transfer = create_transfer(&account);

        let (modified_transfer, _eth_signature) =
            transfer.nonexistent_token(account.eth_account_data.unwrap_eoa_pk(), "ETH", 18);

        assert_ne!(unwrap_transfer(modified_transfer).token, TokenId(0));
    }

    #[test]
    fn bad_eth_signature() {
        let account = create_account();

        let transfer = create_transfer(&account);
        let current_eth_signature = transfer.1.clone();

        let (_modified_transfer, new_eth_signature) =
            transfer.bad_eth_signature(account.eth_account_data.unwrap_eoa_pk(), "ETH", 18);

        assert_ne!(current_eth_signature, new_eth_signature);
    }

    #[test]
    fn bad_zksync_signature() {
        let account = create_account();

        let transfer = create_transfer(&account);
        let current_zksync_signature = unwrap_transfer(transfer.0.clone()).signature;

        let (modified_transfer, _eth_signature) = transfer.bad_zksync_signature();

        assert_ne!(
            current_zksync_signature
                .signature
                .serialize_packed()
                .unwrap(),
            unwrap_transfer(modified_transfer)
                .signature
                .signature
                .serialize_packed()
                .unwrap()
        );
    }
}
