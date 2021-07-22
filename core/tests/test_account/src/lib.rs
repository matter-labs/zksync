// Built-in imports
use std::{fmt, sync::Mutex};
// External uses
use num::BigUint;
// Workspace uses
use zksync_basic_types::H256;
use zksync_crypto::rand::{thread_rng, Rng};
use zksync_crypto::{priv_key_from_fs, PrivateKey};
use zksync_types::{
    tx::{
        ChangePubKey, ChangePubKeyCREATE2Data, ChangePubKeyECDSAData, ChangePubKeyEthAuthData,
        ChangePubKeyType, PackedEthSignature, TimeRange, TxSignature,
    },
    AccountId, Address, Close, ForcedExit, MintNFT, Nonce, Order, PubKeyHash, Swap, TokenId,
    Transfer, Withdraw, WithdrawNFT,
};

#[derive(Debug, Clone)]
pub enum ZkSyncETHAccountData {
    /// Externally Owned Account that have private key
    EOA { eth_private_key: H256 },
    /// Smart contract accounts that can be created with CREATE2
    Create2(ChangePubKeyCREATE2Data),
}

impl ZkSyncETHAccountData {
    pub fn is_eoa(&self) -> bool {
        matches!(self, ZkSyncETHAccountData::EOA { .. })
    }

    pub fn unwrap_eoa_pk(&self) -> H256 {
        match self {
            Self::EOA { eth_private_key } => *eth_private_key,
            _ => panic!("Not an EOA"),
        }
    }

    pub fn is_create2(&self) -> bool {
        matches!(self, ZkSyncETHAccountData::Create2(..))
    }
}

/// Structure used to sign ZKSync transactions, keeps tracks of its nonce internally
pub struct ZkSyncAccount {
    pub private_key: PrivateKey,
    pub pubkey_hash: PubKeyHash,
    pub address: Address,
    pub eth_account_data: ZkSyncETHAccountData,
    account_id: Mutex<Option<AccountId>>,
    nonce: Mutex<Nonce>,
}

impl Clone for ZkSyncAccount {
    fn clone(&self) -> Self {
        Self {
            private_key: priv_key_from_fs(self.private_key.0),
            pubkey_hash: self.pubkey_hash,
            address: self.address,
            eth_account_data: self.eth_account_data.clone(),
            account_id: Mutex::new(*self.account_id.lock().unwrap()),
            nonce: Mutex::new(*self.nonce.lock().unwrap()),
        }
    }
}

impl fmt::Debug for ZkSyncAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // It is OK to disclose the private key contents for a testkit account.
        let mut pk_contents = Vec::new();
        self.private_key
            .write(&mut pk_contents)
            .expect("Failed writing the private key contents");

        f.debug_struct("ZkSyncAccount")
            .field("private_key", &pk_contents)
            .field("pubkey_hash", &self.pubkey_hash)
            .field("address", &self.address)
            .field("eth_account_data", &self.eth_account_data)
            .field("nonce", &self.nonce)
            .finish()
    }
}

impl ZkSyncAccount {
    /// Note: probably not secure, use for testing.
    pub fn rand() -> Self {
        let rng = &mut thread_rng();

        let pk = priv_key_from_fs(rng.gen());
        let (eth_private_key, eth_address) = {
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
        Self::new(
            pk,
            Nonce(0),
            eth_address,
            ZkSyncETHAccountData::EOA { eth_private_key },
        )
    }

    pub fn new(
        private_key: PrivateKey,
        nonce: Nonce,
        address: Address,
        eth_account_data: ZkSyncETHAccountData,
    ) -> Self {
        let pubkey_hash = PubKeyHash::from_privkey(&private_key);
        if let ZkSyncETHAccountData::EOA { eth_private_key } = &eth_account_data {
            assert_eq!(
                address,
                PackedEthSignature::address_from_private_key(&eth_private_key)
                    .expect("private key is incorrect"),
                "address should correspond to private key"
            );
        }
        Self {
            account_id: Mutex::new(None),
            address,
            private_key,
            pubkey_hash,
            eth_account_data,
            nonce: Mutex::new(nonce),
        }
    }

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
    pub fn sign_mint_nft(
        &self,
        fee_token: TokenId,
        token_symbol: &str,
        content_hash: H256,
        fee: BigUint,
        recipient: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> (MintNFT, Option<PackedEthSignature>) {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let mint_nft = MintNFT::new_signed(
            self.account_id
                .lock()
                .unwrap()
                .expect("can't sign tx without account id"),
            self.address,
            content_hash,
            *recipient,
            fee,
            fee_token,
            nonce.unwrap_or_else(|| *stored_nonce),
            &self.private_key,
        )
        .expect("Failed to sign mint nft");

        if increment_nonce {
            **stored_nonce += 1;
        }

        let eth_signature =
            if let ZkSyncETHAccountData::EOA { eth_private_key } = &self.eth_account_data {
                let message = mint_nft.get_ethereum_sign_message(token_symbol, 18);
                Some(
                    PackedEthSignature::sign(&eth_private_key, &message.as_bytes())
                        .expect("Signing the mint nft unexpectedly failed"),
                )
            } else {
                None
            };
        (mint_nft, eth_signature)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sign_withdraw_nft(
        &self,
        token: TokenId,
        fee_token: TokenId,
        token_symbol: &str,
        fee: BigUint,
        recipient: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        time_range: TimeRange,
    ) -> (WithdrawNFT, Option<PackedEthSignature>) {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let withdraw_nft = WithdrawNFT::new_signed(
            self.account_id
                .lock()
                .unwrap()
                .expect("can't sign tx without account id"),
            self.address,
            *recipient,
            token,
            fee_token,
            fee,
            nonce.unwrap_or_else(|| *stored_nonce),
            time_range,
            &self.private_key,
        )
        .expect("Failed to sign withdraw nft");

        if increment_nonce {
            **stored_nonce += 1;
        }

        let eth_signature =
            if let ZkSyncETHAccountData::EOA { eth_private_key } = &self.eth_account_data {
                let message = withdraw_nft.get_ethereum_sign_message(token_symbol, 18);
                Some(
                    PackedEthSignature::sign(&eth_private_key, &message.as_bytes())
                        .expect("Signing the withdraw nft unexpectedly failed"),
                )
            } else {
                None
            };
        (withdraw_nft, eth_signature)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sign_order(
        &self,
        token_sell: TokenId,
        token_buy: TokenId,
        price_sell: BigUint,
        price_buy: BigUint,
        amount: BigUint,
        recipient: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        time_range: TimeRange,
    ) -> Order {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let order = Order::new_signed(
            self.get_account_id()
                .expect("can't sign tx without account id"),
            *recipient,
            nonce.unwrap_or_else(|| *stored_nonce),
            token_sell,
            token_buy,
            (price_sell, price_buy),
            amount,
            time_range,
            &self.private_key,
        )
        .expect("Failed to sign order");

        if increment_nonce {
            **stored_nonce += 1;
        }

        order
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sign_swap(
        &self,
        orders: (Order, Order),
        amounts: (BigUint, BigUint),
        nonce: Option<Nonce>,
        increment_nonce: bool,
        fee_token: TokenId,
        fee_token_symbol: &str,
        fee: BigUint,
    ) -> (Swap, Option<PackedEthSignature>) {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let swap = Swap::new_signed(
            self.get_account_id()
                .expect("can't sign tx without account id"),
            self.address,
            nonce.unwrap_or_else(|| *stored_nonce),
            orders,
            amounts,
            fee,
            fee_token,
            &self.private_key,
        )
        .expect("Failed to sign swap");

        if increment_nonce {
            **stored_nonce += 1;
        }

        let eth_signature =
            if let ZkSyncETHAccountData::EOA { eth_private_key } = &self.eth_account_data {
                let message = swap.get_ethereum_sign_message(fee_token_symbol, 18);
                Some(
                    PackedEthSignature::sign(&eth_private_key, &message.as_bytes())
                        .expect("Signing the swap unexpectedly failed"),
                )
            } else {
                None
            };
        (swap, eth_signature)
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
        time_range: TimeRange,
    ) -> (Transfer, Option<PackedEthSignature>) {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let transfer = Transfer::new_signed(
            self.account_id
                .lock()
                .unwrap()
                .expect("can't sign tx without account id"),
            self.address,
            *to,
            token_id,
            amount,
            fee,
            nonce.unwrap_or_else(|| *stored_nonce),
            time_range,
            &self.private_key,
        )
        .expect("Failed to sign transfer");

        if increment_nonce {
            **stored_nonce += 1;
        }

        let eth_signature =
            if let ZkSyncETHAccountData::EOA { eth_private_key } = &self.eth_account_data {
                let message = transfer.get_ethereum_sign_message(token_symbol, 18);
                Some(
                    PackedEthSignature::sign(&eth_private_key, &message.as_bytes())
                        .expect("Signing the transfer unexpectedly failed"),
                )
            } else {
                None
            };
        (transfer, eth_signature)
    }

    pub fn sign_forced_exit(
        &self,
        token_id: TokenId,
        fee: BigUint,
        target: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        time_range: TimeRange,
    ) -> ForcedExit {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let forced_exit = ForcedExit::new_signed(
            self.account_id
                .lock()
                .unwrap()
                .expect("can't sign tx without account id"),
            *target,
            token_id,
            fee,
            nonce.unwrap_or_else(|| *stored_nonce),
            time_range,
            &self.private_key,
        )
        .expect("Failed to sign forced exit");

        if increment_nonce {
            **stored_nonce += 1;
        }

        forced_exit
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
        time_range: TimeRange,
    ) -> (Withdraw, Option<PackedEthSignature>) {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let withdraw = Withdraw::new_signed(
            self.account_id
                .lock()
                .unwrap()
                .expect("can't sign tx without account id"),
            self.address,
            *eth_address,
            token_id,
            amount,
            fee,
            nonce.unwrap_or_else(|| *stored_nonce),
            time_range,
            &self.private_key,
        )
        .expect("Failed to sign withdraw");

        if increment_nonce {
            **stored_nonce += 1;
        }

        let eth_signature =
            if let ZkSyncETHAccountData::EOA { eth_private_key } = &self.eth_account_data {
                let message = withdraw.get_ethereum_sign_message(token_symbol, 18);
                Some(
                    PackedEthSignature::sign(eth_private_key, &message.as_bytes())
                        .expect("Signing the withdraw unexpectedly failed"),
                )
            } else {
                None
            };
        (withdraw, eth_signature)
    }

    pub fn sign_close(&self, nonce: Option<Nonce>, increment_nonce: bool) -> Close {
        let mut stored_nonce = self.nonce.lock().unwrap();
        let mut close = Close {
            account: self.address,
            nonce: nonce.unwrap_or_else(|| *stored_nonce),
            signature: TxSignature::default(),
            time_range: Default::default(),
        };
        close.signature = TxSignature::sign_musig(&self.private_key, &close.get_bytes());

        if increment_nonce {
            **stored_nonce += 1;
        }
        close
    }

    pub fn sign_change_pubkey_tx(
        &self,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        fee_token: TokenId,
        fee: BigUint,
        auth_type: ChangePubKeyType,
        time_range: TimeRange,
    ) -> ChangePubKey {
        let account_id = self
            .account_id
            .lock()
            .unwrap()
            .expect("can't sign tx withoud account id");
        let mut stored_nonce = self.nonce.lock().unwrap();
        let nonce = nonce.unwrap_or_else(|| *stored_nonce);

        let mut change_pubkey = ChangePubKey::new_signed(
            account_id,
            self.address,
            self.pubkey_hash,
            fee_token,
            fee,
            nonce,
            time_range,
            None,
            &self.private_key,
        )
        .expect("Can't sign ChangePubKey operation");

        let eth_auth_data = match auth_type {
            ChangePubKeyType::Onchain => ChangePubKeyEthAuthData::Onchain,
            ChangePubKeyType::ECDSA => {
                if let ZkSyncETHAccountData::EOA { eth_private_key } = &self.eth_account_data {
                    let sign_bytes = change_pubkey
                        .get_eth_signed_data()
                        .expect("Failed to construct change pubkey signed message.");
                    let eth_signature = PackedEthSignature::sign(eth_private_key, &sign_bytes)
                        .expect("Signature should succeed");
                    ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData {
                        eth_signature,
                        batch_hash: H256::zero(),
                    })
                } else {
                    panic!("ECDSA ChangePubKey can only be executed for EOA account");
                }
            }
            ChangePubKeyType::CREATE2 => {
                if let ZkSyncETHAccountData::Create2(create2_data) = &self.eth_account_data {
                    ChangePubKeyEthAuthData::CREATE2(create2_data.clone())
                } else {
                    panic!("CREATE2 ChangePubKey can only be executed for CREATE2 account");
                }
            }
        };
        change_pubkey.eth_auth_data = Some(eth_auth_data);

        assert!(
            change_pubkey.is_eth_auth_data_valid(),
            "eth auth data is incorrect"
        );

        if increment_nonce {
            **stored_nonce += 1;
        }

        change_pubkey
    }

    pub fn try_get_eth_private_key(&self) -> Option<&H256> {
        if let ZkSyncETHAccountData::EOA { eth_private_key } = &self.eth_account_data {
            Some(eth_private_key)
        } else {
            None
        }
    }
}
