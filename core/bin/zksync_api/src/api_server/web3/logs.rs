// Built-in uses
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
// External uses
use ethabi::{encode, Contract, Token as AbiToken};
use jsonrpc_core::{Error, Result};
use num::{BigUint, Zero};
// Workspace uses
use zksync_storage::StorageProcessor;
use zksync_token_db_cache::TokenDBCache;
use zksync_types::{Nonce, Token, TokenId, TokenKind, ZkSyncOp, NFT};
// Local uses
use super::{
    converter::{log, u256_from_biguint},
    types::{Bytes, CommonLogData, Event, Log, H160, H256, U256},
    NFT_FACTORY_ADDRESS, ZKSYNC_PROXY_ADDRESS,
};

#[derive(Debug, Clone)]
pub struct LogsHelper {
    topic_by_event: HashMap<Event, H256>,
    tokens: TokenDBCache,
    zksync_proxy_address: H160,
    nft_factory_address: H160,
}

impl LogsHelper {
    pub fn new() -> Self {
        let mut path = PathBuf::new();
        path.push(std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| "/".to_string()));
        path.push("etc/web3-abi");

        let proxy_abi = std::fs::File::open(path.join("ZkSyncProxy.json")).unwrap();
        let proxy_contract = Contract::load(proxy_abi).unwrap();

        let erc20_abi = std::fs::File::open(path.join("ERC20.json")).unwrap();
        let erc20_contract = Contract::load(erc20_abi).unwrap();

        let topic_by_event: HashMap<_, _> = vec![
            (
                Event::ZkSyncTransfer,
                proxy_contract.event("ZkSyncTransfer").unwrap().signature(),
            ),
            (
                Event::ZkSyncWithdraw,
                proxy_contract.event("ZkSyncWithdraw").unwrap().signature(),
            ),
            (
                Event::ZkSyncForcedExit,
                proxy_contract
                    .event("ZkSyncForcedExit")
                    .unwrap()
                    .signature(),
            ),
            (
                Event::ZkSyncChangePubKey,
                proxy_contract
                    .event("ZkSyncChangePubKey")
                    .unwrap()
                    .signature(),
            ),
            (
                Event::ZkSyncDeposit,
                proxy_contract.event("ZkSyncDeposit").unwrap().signature(),
            ),
            (
                Event::ZkSyncFullExit,
                proxy_contract.event("ZkSyncFullExit").unwrap().signature(),
            ),
            (
                Event::ZkSyncMintNFT,
                proxy_contract.event("ZkSyncMintNFT").unwrap().signature(),
            ),
            (
                Event::ZkSyncWithdrawNFT,
                proxy_contract
                    .event("ZkSyncWithdrawNFT")
                    .unwrap()
                    .signature(),
            ),
            (
                Event::ZkSyncSwap,
                proxy_contract.event("ZkSyncSwap").unwrap().signature(),
            ),
            (
                Event::ERCTransfer,
                erc20_contract.event("Transfer").unwrap().signature(),
            ),
        ]
        .into_iter()
        .collect();

        Self {
            topic_by_event,
            tokens: TokenDBCache::new(Duration::from_secs(5 * 60)),
            zksync_proxy_address: H160::from_str(ZKSYNC_PROXY_ADDRESS).unwrap(),
            nft_factory_address: H160::from_str(NFT_FACTORY_ADDRESS).unwrap(),
        }
    }

    pub fn topic_by_event(&self, event: Event) -> Option<H256> {
        self.topic_by_event.get(&event).cloned()
    }

    pub async fn zksync_log(
        &self,
        op: ZkSyncOp,
        common_data: CommonLogData,
        storage: &mut StorageProcessor<'_>,
    ) -> Result<Option<Log>> {
        let log_data = match op {
            ZkSyncOp::Transfer(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let data = Self::zksync_transfer_data(
                    op.tx.from,
                    op.tx.to,
                    token.address,
                    u256_from_biguint(op.tx.amount),
                    u256_from_biguint(op.tx.fee),
                );
                Some((Event::ZkSyncTransfer, data))
            }
            ZkSyncOp::TransferToNew(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let data = Self::zksync_transfer_data(
                    op.tx.from,
                    op.tx.to,
                    token.address,
                    u256_from_biguint(op.tx.amount),
                    u256_from_biguint(op.tx.fee),
                );
                Some((Event::ZkSyncTransfer, data))
            }
            ZkSyncOp::Withdraw(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let data = Self::zksync_withdraw_data(
                    op.tx.from,
                    op.tx.to,
                    token.address,
                    u256_from_biguint(op.tx.amount),
                    u256_from_biguint(op.tx.fee),
                );
                Some((Event::ZkSyncWithdraw, data))
            }
            ZkSyncOp::ForcedExit(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let initiator = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.initiator_account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?;
                let data = Self::zksync_forced_exit_data(
                    initiator,
                    op.tx.target,
                    token.address,
                    u256_from_biguint(op.tx.fee),
                );
                Some((Event::ZkSyncForcedExit, data))
            }
            ZkSyncOp::ChangePubKeyOffchain(op) => {
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                let data = Self::zksync_change_pub_key_data(
                    op.tx.account,
                    op.tx.new_pk_hash.data,
                    fee_token.address,
                    u256_from_biguint(op.tx.fee),
                );
                Some((Event::ZkSyncChangePubKey, data))
            }
            ZkSyncOp::MintNFTOp(op) => {
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                let nft = self
                    .get_nft_by_creator_and_nonce(storage, op.tx.creator_address, op.tx.nonce)
                    .await?;
                let data = Self::zksync_mint_nft_data(
                    nft.id.0.into(),
                    op.tx.creator_id.0.into(),
                    op.tx.creator_address,
                    op.tx.content_hash,
                    op.tx.recipient,
                    u256_from_biguint(op.tx.fee),
                    fee_token.address,
                );
                Some((Event::ZkSyncMintNFT, data))
            }
            ZkSyncOp::WithdrawNFT(op) => {
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                let nft = self.get_nft_by_id(storage, op.tx.token).await?;
                let data = Self::zksync_withdraw_nft_data(
                    op.tx.from,
                    op.tx.to,
                    U256::from(nft.id.0),
                    nft.address,
                    fee_token.address,
                    u256_from_biguint(op.tx.fee),
                    U256::from(nft.creator_id.0),
                    nft.creator_address,
                    U256::from(nft.serial_id),
                    nft.content_hash,
                );
                Some((Event::ZkSyncWithdrawNFT, data))
            }
            ZkSyncOp::Swap(op) => {
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                let token1 = self
                    .get_token_by_id(storage, op.tx.orders.0.token_buy)
                    .await?;
                let token2 = self
                    .get_token_by_id(storage, op.tx.orders.0.token_sell)
                    .await?;
                let account1 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.accounts.0)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?;
                let account2 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.accounts.1)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?;
                let data = Self::zksync_swap_data(
                    op.tx.submitter_address,
                    account1,
                    account2,
                    op.tx.orders.0.recipient_address,
                    op.tx.orders.1.recipient_address,
                    fee_token.address,
                    token1.address,
                    token2.address,
                    u256_from_biguint(op.tx.fee),
                    u256_from_biguint(op.tx.amounts.0),
                    u256_from_biguint(op.tx.amounts.1),
                );
                Some((Event::ZkSyncSwap, data))
            }
            ZkSyncOp::Deposit(op) => {
                let token = self.get_token_by_id(storage, op.priority_op.token).await?;
                let data = Self::zksync_deposit_data(
                    op.priority_op.from,
                    op.priority_op.to,
                    token.address,
                    u256_from_biguint(op.priority_op.amount),
                );
                Some((Event::ZkSyncDeposit, data))
            }
            ZkSyncOp::FullExit(op) => {
                let token = self.get_token_by_id(storage, op.priority_op.token).await?;
                let amount = op.withdraw_amount.unwrap_or_default().0;
                let account = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.priority_op.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?;
                let data =
                    Self::zksync_full_exit_data(account, token.address, u256_from_biguint(amount));
                Some((Event::ZkSyncFullExit, data))
            }
            _ => None,
        };
        let log = log_data.map(|(event, data)| {
            log(
                self.zksync_proxy_address,
                self.topic_by_event(event).unwrap(),
                data,
                common_data,
                0u8.into(),
            )
        });
        Ok(log)
    }

    /// Returns info for erc logs produced by operation
    /// Info structure: (token, from, to, amount)
    async fn erc_logs_info(
        &self,
        op: ZkSyncOp,
        storage: &mut StorageProcessor<'_>,
    ) -> Result<Vec<(Token, H160, H160, BigUint)>> {
        let mut result = Vec::new();
        match op {
            ZkSyncOp::Transfer(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                result.push((token.clone(), op.tx.from, op.tx.to, op.tx.amount));
                result.push((token, op.tx.from, H160::zero(), op.tx.fee));
            }
            ZkSyncOp::TransferToNew(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                result.push((token.clone(), op.tx.from, op.tx.to, op.tx.amount));
                result.push((token, op.tx.from, H160::zero(), op.tx.fee));
            }
            ZkSyncOp::Withdraw(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                result.push((token.clone(), op.tx.from, H160::zero(), op.tx.amount));
                result.push((token, op.tx.from, H160::zero(), op.tx.fee));
            }
            ZkSyncOp::ForcedExit(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let initiator = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.initiator_account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?;
                let amount = op.withdraw_amount.unwrap_or_default().0;
                result.push((token.clone(), op.tx.target, H160::zero(), amount));
                result.push((token, initiator, H160::zero(), op.tx.fee));
            }
            ZkSyncOp::ChangePubKeyOffchain(op) => {
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                result.push((fee_token, op.tx.account, H160::zero(), op.tx.fee));
            }
            ZkSyncOp::MintNFTOp(op) => {
                let nft = self
                    .get_nft_by_creator_and_nonce(storage, op.tx.creator_address, op.tx.nonce)
                    .await?;
                let token = Token::new_nft(nft.id, "");
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                result.push((token, H160::zero(), op.tx.recipient, BigUint::from(1u8)));
                result.push((fee_token, op.tx.creator_address, H160::zero(), op.tx.fee));
            }
            ZkSyncOp::WithdrawNFT(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                result.push((token, op.tx.from, H160::zero(), BigUint::from(1u8)));
                result.push((fee_token, op.tx.from, H160::zero(), op.tx.fee));
            }
            ZkSyncOp::Swap(op) => {
                let token1 = self
                    .get_token_by_id(storage, op.tx.orders.0.token_buy)
                    .await?;
                let token2 = self
                    .get_token_by_id(storage, op.tx.orders.0.token_sell)
                    .await?;
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                let from1 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.orders.0.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?;
                let from2 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.orders.1.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?;
                result.push((
                    token1,
                    from1,
                    op.tx.orders.1.recipient_address,
                    op.tx.amounts.0,
                ));
                result.push((
                    token2,
                    from2,
                    op.tx.orders.0.recipient_address,
                    op.tx.amounts.1,
                ));
                result.push((fee_token, op.tx.submitter_address, H160::zero(), op.tx.fee));
            }
            ZkSyncOp::Deposit(op) => {
                let token = self.get_token_by_id(storage, op.priority_op.token).await?;
                result.push((
                    token,
                    H160::zero(),
                    op.priority_op.to,
                    op.priority_op.amount,
                ));
            }
            ZkSyncOp::FullExit(op) => {
                let token = self.get_token_by_id(storage, op.priority_op.token).await?;
                let from = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.priority_op.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?;
                result.push((
                    token,
                    from,
                    H160::zero(),
                    op.withdraw_amount.unwrap_or_default().0,
                ));
            }
            _ => {}
        }
        Ok(result)
    }

    pub async fn erc_logs(
        &self,
        op: ZkSyncOp,
        common_data: CommonLogData,
        storage: &mut StorageProcessor<'_>,
    ) -> Result<Vec<Log>> {
        let mut logs = Vec::new();
        // The index is equal to 1 because zksync log has index 0.
        let mut index = 1u8.into();

        let info = self.erc_logs_info(op, storage).await?;
        logs.extend(info.into_iter().filter_map(|(token, from, to, amount)| {
            if amount.is_zero() {
                None
            } else {
                Some(self.erc_transfer(token, from, to, amount, common_data, &mut index))
            }
        }));

        Ok(logs)
    }

    async fn get_token_by_id(
        &self,
        storage: &mut StorageProcessor<'_>,
        id: TokenId,
    ) -> Result<Token> {
        self.tokens
            .get_token(storage, id)
            .await
            .map_err(|_| Error::internal_error())?
            .ok_or_else(Error::internal_error)
    }

    async fn get_nft_by_id(&self, storage: &mut StorageProcessor<'_>, id: TokenId) -> Result<NFT> {
        self.tokens
            .get_nft_by_id(storage, id)
            .await
            .map_err(|_| Error::internal_error())?
            .ok_or_else(Error::internal_error)
    }

    async fn get_nft_by_creator_and_nonce(
        &self,
        storage: &mut StorageProcessor<'_>,
        creator_address: H160,
        nonce: Nonce,
    ) -> Result<NFT> {
        storage
            .chain()
            .state_schema()
            .get_mint_nft_update_by_creator_and_nonce(creator_address, nonce)
            .await
            .map_err(|_| Error::internal_error())?
            .ok_or_else(Error::internal_error)
    }

    fn erc_transfer(
        &self,
        token: Token,
        from: H160,
        to: H160,
        amount: BigUint,
        common_data: CommonLogData,
        transaction_log_index: &mut U256,
    ) -> Log {
        // According to specifications, amount is added to transfer log data for ERC20 tokens
        // and token ID is added instead for ERC721 tokens.
        let (contract_address, amount_or_id) = match token.kind {
            TokenKind::NFT => (self.nft_factory_address, token.id.0.into()),
            _ => (token.address, u256_from_biguint(amount)),
        };
        let data = Self::erc_transfer_data(from, to, amount_or_id);
        let log = log(
            contract_address,
            self.topic_by_event(Event::ERCTransfer).unwrap(),
            data,
            common_data,
            *transaction_log_index,
        );
        *transaction_log_index += 1u8.into();
        log
    }

    fn erc_transfer_data(from: H160, to: H160, amount_or_id: U256) -> Bytes {
        let bytes = encode(&[
            AbiToken::Address(from),
            AbiToken::Address(to),
            AbiToken::Uint(amount_or_id),
        ]);
        bytes.into()
    }

    fn zksync_transfer_data(from: H160, to: H160, token: H160, amount: U256, fee: U256) -> Bytes {
        let bytes = encode(&[
            AbiToken::Address(from),
            AbiToken::Address(to),
            AbiToken::Address(token),
            AbiToken::Uint(amount),
            AbiToken::Uint(fee),
        ]);
        bytes.into()
    }

    fn zksync_withdraw_data(from: H160, to: H160, token: H160, amount: U256, fee: U256) -> Bytes {
        Self::zksync_transfer_data(from, to, token, amount, fee)
    }

    fn zksync_forced_exit_data(initiator: H160, target: H160, token: H160, fee: U256) -> Bytes {
        let bytes = encode(&[
            AbiToken::Address(initiator),
            AbiToken::Address(target),
            AbiToken::Address(token),
            AbiToken::Uint(fee),
        ]);
        bytes.into()
    }

    fn zksync_change_pub_key_data(
        account_address: H160,
        new_pub_key_hash: [u8; 20],
        token: H160,
        fee: U256,
    ) -> Bytes {
        let bytes = encode(&[
            AbiToken::Address(account_address),
            AbiToken::FixedBytes(new_pub_key_hash.to_vec()),
            AbiToken::Address(token),
            AbiToken::Uint(fee),
        ]);
        bytes.into()
    }

    fn zksync_mint_nft_data(
        token_id: U256,
        creator_id: U256,
        creator_address: H160,
        content_hash: H256,
        recipient: H160,
        fee: U256,
        fee_token: H160,
    ) -> Bytes {
        let bytes = encode(&[
            AbiToken::Uint(token_id),
            AbiToken::Uint(creator_id),
            AbiToken::Address(creator_address),
            AbiToken::FixedBytes(content_hash.as_bytes().to_vec()),
            AbiToken::Address(recipient),
            AbiToken::Uint(fee),
            AbiToken::Address(fee_token),
        ]);
        bytes.into()
    }

    #[allow(clippy::too_many_arguments)]
    fn zksync_withdraw_nft_data(
        from: H160,
        to: H160,
        token_id: U256,
        token_address: H160,
        fee_token: H160,
        fee: U256,
        creator_id: U256,
        creator_address: H160,
        serial_id: U256,
        content_hash: H256,
    ) -> Bytes {
        let bytes = encode(&[
            AbiToken::Address(from),
            AbiToken::Address(to),
            AbiToken::Uint(token_id),
            AbiToken::Address(token_address),
            AbiToken::Address(fee_token),
            AbiToken::Uint(fee),
            AbiToken::Uint(creator_id),
            AbiToken::Address(creator_address),
            AbiToken::Uint(serial_id),
            AbiToken::FixedBytes(content_hash.as_bytes().to_vec()),
        ]);
        bytes.into()
    }

    #[allow(clippy::too_many_arguments)]
    fn zksync_swap_data(
        initiator: H160,
        account_address1: H160,
        account_address2: H160,
        recipient1: H160,
        recipient2: H160,
        fee_token: H160,
        token1: H160,
        token2: H160,
        fee: U256,
        amount1: U256,
        amount2: U256,
    ) -> Bytes {
        let bytes = encode(&[
            AbiToken::Address(initiator),
            AbiToken::Address(account_address1),
            AbiToken::Address(account_address2),
            AbiToken::Address(recipient1),
            AbiToken::Address(recipient2),
            AbiToken::Address(fee_token),
            AbiToken::Address(token1),
            AbiToken::Address(token2),
            AbiToken::Uint(fee),
            AbiToken::Uint(amount1),
            AbiToken::Uint(amount2),
        ]);
        bytes.into()
    }

    fn zksync_deposit_data(from: H160, to: H160, token: H160, amount: U256) -> Bytes {
        let bytes = encode(&[
            AbiToken::Address(from),
            AbiToken::Address(to),
            AbiToken::Address(token),
            AbiToken::Uint(amount),
        ]);
        bytes.into()
    }

    fn zksync_full_exit_data(account_address: H160, token: H160, amount: U256) -> Bytes {
        let bytes = encode(&[
            AbiToken::Address(account_address),
            AbiToken::Address(token),
            AbiToken::Uint(amount),
        ]);
        bytes.into()
    }
}
