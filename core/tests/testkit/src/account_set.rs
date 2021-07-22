use crate::eth_account::EthereumAccount;
use crate::zksync_account::ZkSyncAccount;
use num::BigUint;
use web3::types::{TransactionReceipt, H256, U64};
use zksync_crypto::rand::Rng;
use zksync_types::tx::{ChangePubKeyType, TimeRange};
use zksync_types::{AccountId, Address, Nonce, PriorityOp, TokenId, ZkSyncTx};

use crate::types::*;

/// Account set is used to create transactions using stored account
/// in a convenient way
#[derive(Clone)]
pub struct AccountSet {
    pub eth_accounts: Vec<EthereumAccount>,
    pub zksync_accounts: Vec<ZkSyncAccount>,
    pub fee_account_id: ZKSyncAccountId,
}
impl AccountSet {
    /// Create deposit from eth account to zksync account
    pub async fn deposit(
        &self,
        from: ETHAccountId,
        to: ZKSyncAccountId,
        token: Option<Address>, // None for ETH
        amount: BigUint,
    ) -> (Vec<TransactionReceipt>, PriorityOp) {
        let from = &self.eth_accounts[from.0];
        let to = &self.zksync_accounts[to.0];

        if let Some(address) = token {
            from.deposit_erc20(address, amount, &to.address)
                .await
                .expect("erc20 deposit should not fail")
        } else {
            from.deposit_eth(amount, &to.address, None)
                .await
                .expect("eth deposit should not fail")
        }
    }

    pub async fn deposit_to_random(
        &self,
        from: ETHAccountId,
        token: Option<Address>, // None for ETH
        amount: BigUint,
        rng: &mut impl Rng,
    ) -> (Vec<TransactionReceipt>, PriorityOp) {
        let from = &self.eth_accounts[from.0];
        let to_address = Address::from_slice(&rng.gen::<[u8; 20]>());

        if let Some(address) = token {
            from.deposit_erc20(address, amount, &to_address)
                .await
                .expect("erc20 deposit should not fail")
        } else {
            from.deposit_eth(amount, &to_address, None)
                .await
                .expect("eth deposit should not fail")
        }
    }

    /// Create signed mint nft between zksync accounts
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub fn mint_nft(
        &self,
        creator: ZKSyncAccountId,
        recipient: ZKSyncAccountId,
        fee_token: Token,
        content_hash: H256,
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> ZkSyncTx {
        let creator = &self.zksync_accounts[creator.0];
        let recipient = &self.zksync_accounts[recipient.0];

        ZkSyncTx::MintNFT(Box::new(
            creator
                .sign_mint_nft(
                    fee_token.0,
                    "",
                    content_hash,
                    fee,
                    &recipient.address,
                    nonce,
                    increment_nonce,
                )
                .0,
        ))
    }
    /// Create signed transfer between zksync accounts
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub fn transfer(
        &self,
        from: ZKSyncAccountId,
        to: ZKSyncAccountId,
        token_id: Token,
        amount: BigUint,
        fee: BigUint,
        nonce: Option<Nonce>,
        time_range: TimeRange,
        increment_nonce: bool,
    ) -> ZkSyncTx {
        let from = &self.zksync_accounts[from.0];
        let to = &self.zksync_accounts[to.0];

        ZkSyncTx::Transfer(Box::new(
            from.sign_transfer(
                token_id.0,
                "",
                amount,
                fee,
                &to.address,
                nonce,
                increment_nonce,
                time_range,
            )
            .0,
        ))
    }

    /// Create signed transfer between zksync accounts
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub fn transfer_to_new_random(
        &self,
        from: ZKSyncAccountId,
        token_id: Token,
        amount: BigUint,
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        rng: &mut impl Rng,
    ) -> ZkSyncTx {
        let from = &self.zksync_accounts[from.0];

        let to_address = Address::from_slice(&rng.gen::<[u8; 20]>());

        ZkSyncTx::Transfer(Box::new(
            from.sign_transfer(
                token_id.0,
                "",
                amount,
                fee,
                &to_address,
                nonce,
                increment_nonce,
                Default::default(),
            )
            .0,
        ))
    }

    /// Create withdraw from zksync account to eth account
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub fn withdraw(
        &self,
        from: ZKSyncAccountId,
        to: ETHAccountId,
        token_id: Token,
        amount: BigUint,
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        time_range: TimeRange,
    ) -> ZkSyncTx {
        let from = &self.zksync_accounts[from.0];
        let to = &self.eth_accounts[to.0];

        ZkSyncTx::Withdraw(Box::new(
            from.sign_withdraw(
                token_id.0,
                "",
                amount,
                fee,
                &to.address,
                nonce,
                increment_nonce,
                time_range,
            )
            .0,
        ))
    }

    /// Create forced exit for zksync account
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub fn forced_exit(
        &self,
        initiator: ZKSyncAccountId,
        target: ZKSyncAccountId,
        token_id: Token,
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        time_range: TimeRange,
    ) -> ZkSyncTx {
        let from = &self.zksync_accounts[initiator.0];
        let target = &self.zksync_accounts[target.0];
        ZkSyncTx::ForcedExit(Box::new(from.sign_forced_exit(
            token_id.0,
            fee,
            &target.address,
            nonce,
            increment_nonce,
            time_range,
        )))
    }

    /// Create withdraw from zksync account to random eth account
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub fn withdraw_nft(
        &self,
        from: ZKSyncAccountId,
        token_id: Token,
        fee_token_id: Token,
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        rng: &mut impl Rng,
    ) -> ZkSyncTx {
        let from = &self.zksync_accounts[from.0];
        let to_address = Address::from_slice(&rng.gen::<[u8; 20]>());

        ZkSyncTx::WithdrawNFT(Box::new(
            from.sign_withdraw_nft(
                token_id.0,
                fee_token_id.0,
                "",
                fee,
                &to_address,
                nonce,
                increment_nonce,
                Default::default(),
            )
            .0,
        ))
    }
    /// Create withdraw from zksync account to random eth account
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub fn withdraw_to_random(
        &self,
        from: ZKSyncAccountId,
        token_id: Token,
        amount: BigUint,
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        rng: &mut impl Rng,
    ) -> ZkSyncTx {
        let from = &self.zksync_accounts[from.0];
        let to_address = Address::from_slice(&rng.gen::<[u8; 20]>());

        ZkSyncTx::Withdraw(Box::new(
            from.sign_withdraw(
                token_id.0,
                "",
                amount,
                fee,
                &to_address,
                nonce,
                increment_nonce,
                Default::default(),
            )
            .0,
        ))
    }

    /// Create full exit from zksync account to eth account
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub async fn full_exit(
        &self,
        post_by: ETHAccountId,
        token_address: Address,
        account_id: AccountId,
    ) -> (TransactionReceipt, PriorityOp) {
        self.eth_accounts[post_by.0]
            .full_exit(account_id, token_address)
            .await
            .expect("FullExit eth call failed")
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn change_pubkey_with_onchain_auth(
        &self,
        eth_account: ETHAccountId,
        zksync_signer: ZKSyncAccountId,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        time_range: TimeRange,
    ) -> ZkSyncTx {
        let zksync_account = &self.zksync_accounts[zksync_signer.0];
        let auth_nonce = nonce.unwrap_or_else(|| zksync_account.nonce());

        let eth_account = &self.eth_accounts[eth_account.0];
        let tx_receipt = eth_account
            .auth_fact(&zksync_account.pubkey_hash.data, auth_nonce)
            .await
            .expect("Auth pubkey fail");
        assert_eq!(tx_receipt.status, Some(U64::from(1)), "Auth pubkey fail");
        ZkSyncTx::ChangePubKey(Box::new(zksync_account.sign_change_pubkey_tx(
            nonce,
            increment_nonce,
            fee_token,
            fee,
            ChangePubKeyType::Onchain,
            time_range,
        )))
    }

    pub fn change_pubkey_with_tx(
        &self,
        zksync_signer: ZKSyncAccountId,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        time_range: TimeRange,
    ) -> ZkSyncTx {
        let zksync_account = &self.zksync_accounts[zksync_signer.0];
        ZkSyncTx::ChangePubKey(Box::new(zksync_account.sign_change_pubkey_tx(
            nonce,
            increment_nonce,
            fee_token,
            fee,
            if zksync_account.eth_account_data.is_eoa() {
                ChangePubKeyType::ECDSA
            } else if zksync_account.eth_account_data.is_create2() {
                ChangePubKeyType::CREATE2
            } else {
                panic!("Not supported, use onchain change pubkey");
            },
            time_range,
        )))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn swap(
        &self,
        accounts: (ZKSyncAccountId, ZKSyncAccountId),
        recipients: (ZKSyncAccountId, ZKSyncAccountId),
        submitter: ZKSyncAccountId,
        tokens: (Token, Token, Token),
        amounts: (BigUint, BigUint),
        fee: BigUint,
        nonce: Option<Nonce>,
        increment_nonce: bool,
        time_range: TimeRange,
    ) -> ZkSyncTx {
        let accounts = (
            &self.zksync_accounts[accounts.0 .0],
            &self.zksync_accounts[accounts.1 .0],
            &self.zksync_accounts[recipients.0 .0],
            &self.zksync_accounts[recipients.1 .0],
            &self.zksync_accounts[submitter.0],
        );

        let order_0 = accounts.0.sign_order(
            tokens.0 .0,
            tokens.1 .0,
            amounts.0.clone(),
            amounts.1.clone(),
            amounts.0.clone(),
            &accounts.2.address,
            None,
            true,
            time_range,
        );

        let order_1 = accounts.1.sign_order(
            tokens.1 .0,
            tokens.0 .0,
            amounts.1.clone(),
            amounts.0.clone(),
            amounts.1.clone(),
            &accounts.3.address,
            None,
            true,
            time_range,
        );

        ZkSyncTx::Swap(Box::new(
            accounts
                .4
                .sign_swap(
                    (order_0, order_1),
                    amounts,
                    nonce,
                    increment_nonce,
                    tokens.2 .0,
                    "",
                    fee,
                )
                .0,
        ))
    }
}
