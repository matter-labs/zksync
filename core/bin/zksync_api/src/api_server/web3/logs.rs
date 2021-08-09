// Built-in uses
use std::collections::HashMap;
use std::str::FromStr;
// External uses
use ethabi::{encode, Token as AbiToken};
use jsonrpc_core::Error;
use num::BigUint;
use tiny_keccak::keccak256;
// Workspace uses
use zksync_storage::StorageProcessor;
use zksync_types::{Token, TokenId, ZkSyncOp, NFT};
// Local uses
use super::converter::{log, u256_from_biguint};
use super::types::{Bytes, CommonLogData, Event, Log, H160, H256, U256};
use crate::utils::token_db_cache::TokenDBCache;

#[derive(Debug, Clone)]
pub struct LogsHelper {
    topic_by_event: HashMap<Event, H256>,
    event_by_topic: HashMap<H256, Event>,
    tokens: TokenDBCache,
    pub zksync_proxy_address: H160,
}

impl LogsHelper {
    pub fn new() -> Self {
        let data = vec![
            (Event::ZkSyncTransfer, "ZkSyncTransfer(address,address,address,uint256,uint256)"),
            (Event::ZkSyncWithdraw, "ZkSyncWithdraw(address,address,address,uint256,uint256)"),
            (Event::ZkSyncForcedExit, "ZkSyncForcedExit(address,address,address,uint256)"),
            (Event::ZkSyncChangePubKey, "ZkSyncChangePubKey(address,bytes20,address,uint256)"),
            (Event::ZkSyncDeposit, "ZkSyncDeposit(address,address,address,uint256)"),
            (Event::ZkSyncFullExit, "ZkSyncFullExit(address,address,uint256)"),
            (Event::ZkSyncMintNFT, "ZkSyncMintNFT(uint32,address,address,bytes32,address,uint256)"),
            (Event::ZkSyncWithdrawNFT, "ZkSyncWithdrawNFT(address,address,address,address,uint256,uint32,address,uint32,bytes32)"),
            (Event::ZkSyncSwap, "ZkSyncSwap(address,address,address,address,address,address,address,address,uint256,uint256,uint256)"),
            (Event::ERCTransfer, "Transfer(address,address,uint256)"),
        ];
        let mut topic_by_event = HashMap::new();
        let mut event_by_topic = HashMap::new();

        for (event_name, event_str) in data.into_iter() {
            let topic = H256::from(keccak256(event_str.as_bytes()));
            topic_by_event.insert(event_name, topic);
            event_by_topic.insert(topic, event_name);
        }

        Self {
            topic_by_event,
            event_by_topic,
            tokens: TokenDBCache::new(),
            zksync_proxy_address: H160::from_str("1000000000000000000000000000000000000000")
                .unwrap(),
        }
    }

    pub fn event_by_topic(&self, topic: &H256) -> Option<Event> {
        self.event_by_topic.get(topic).cloned()
    }

    pub fn topic_by_event(&self, event: Event) -> Option<H256> {
        self.topic_by_event.get(&event).cloned()
    }

    pub async fn zksync_log(
        &self,
        op: ZkSyncOp,
        common_data: CommonLogData,
        storage: &mut StorageProcessor<'_>,
    ) -> jsonrpc_core::Result<Option<Log>> {
        let transaction_log_index = Self::zksync_op_log_index(&op);
        let log_data = match op {
            ZkSyncOp::Transfer(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let data = Self::zksync_transfer_data(
                    op.tx.from,
                    op.tx.to,
                    token.address,
                    u256_from_biguint(op.tx.amount)?,
                    u256_from_biguint(op.tx.fee)?,
                );
                Some((Event::ZkSyncTransfer, data))
            }
            ZkSyncOp::TransferToNew(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let data = Self::zksync_transfer_data(
                    op.tx.from,
                    op.tx.to,
                    token.address,
                    u256_from_biguint(op.tx.amount)?,
                    u256_from_biguint(op.tx.fee)?,
                );
                Some((Event::ZkSyncTransfer, data))
            }
            ZkSyncOp::Withdraw(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let data = Self::zksync_withdraw_data(
                    op.tx.from,
                    op.tx.to,
                    token.address,
                    u256_from_biguint(op.tx.amount)?,
                    u256_from_biguint(op.tx.fee)?,
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
                    .expect("Can`t find account in storage");
                let data = Self::zksync_forced_exit_data(
                    initiator,
                    op.tx.target,
                    token.address,
                    u256_from_biguint(op.tx.fee)?,
                );
                Some((Event::ZkSyncForcedExit, data))
            }
            ZkSyncOp::ChangePubKeyOffchain(op) => {
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                let data = Self::zksync_change_pub_key_data(
                    op.tx.account,
                    op.tx.new_pk_hash.data,
                    fee_token.address,
                    u256_from_biguint(op.tx.fee)?,
                );
                Some((Event::ZkSyncChangePubKey, data))
            }
            ZkSyncOp::MintNFTOp(op) => {
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                let data = Self::zksync_mint_nft_data(
                    op.tx.creator_id.0.into(),
                    op.tx.creator_address,
                    op.tx.content_hash,
                    op.tx.recipient,
                    u256_from_biguint(op.tx.fee)?,
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
                    nft.address,
                    fee_token.address,
                    u256_from_biguint(op.tx.fee)?,
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
                    .expect("Can`t find account in storage");
                let account2 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.accounts.1)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .expect("Can`t find account in storage");
                let data = Self::zksync_swap_data(
                    op.tx.submitter_address,
                    account1,
                    account2,
                    op.tx.orders.0.recipient_address,
                    op.tx.orders.1.recipient_address,
                    fee_token.address,
                    token1.address,
                    token2.address,
                    u256_from_biguint(op.tx.fee)?,
                    u256_from_biguint(op.tx.amounts.0)?,
                    u256_from_biguint(op.tx.amounts.1)?,
                );
                Some((Event::ZkSyncSwap, data))
            }
            ZkSyncOp::Deposit(op) => {
                let token = self.get_token_by_id(storage, op.priority_op.token).await?;
                let data = Self::zksync_deposit_data(
                    op.priority_op.from,
                    op.priority_op.to,
                    token.address,
                    u256_from_biguint(op.priority_op.amount)?,
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
                    .expect("Can`t find account in storage");
                let data =
                    Self::zksync_full_exit_data(account, token.address, u256_from_biguint(amount)?);
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
                transaction_log_index,
            )
        });
        Ok(log)
    }

    pub async fn erc_logs(
        &self,
        op: ZkSyncOp,
        common_data: CommonLogData,
        storage: &mut StorageProcessor<'_>,
    ) -> jsonrpc_core::Result<Vec<Log>> {
        let mut logs = Vec::new();
        match op {
            ZkSyncOp::Transfer(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                logs.push(
                    self.erc_transfer(
                        token,
                        op.tx.from,
                        op.tx.to,
                        op.tx.amount,
                        common_data,
                        0u8.into(),
                        storage,
                    )
                    .await?,
                );
            }
            ZkSyncOp::TransferToNew(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                logs.push(
                    self.erc_transfer(
                        token,
                        op.tx.from,
                        op.tx.to,
                        op.tx.amount,
                        common_data,
                        0u8.into(),
                        storage,
                    )
                    .await?,
                );
            }
            ZkSyncOp::Withdraw(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                logs.push(
                    self.erc_transfer(
                        token,
                        op.tx.from,
                        H160::zero(),
                        op.tx.amount,
                        common_data,
                        0u8.into(),
                        storage,
                    )
                    .await?,
                );
            }
            ZkSyncOp::ForcedExit(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                let from = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.initiator_account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .expect("Can`t find account in storage");
                logs.push(
                    self.erc_transfer(
                        token,
                        from,
                        H160::zero(),
                        op.withdraw_amount.unwrap_or_default().0,
                        common_data,
                        0u8.into(),
                        storage,
                    )
                    .await?,
                );
            }
            ZkSyncOp::MintNFTOp(_op) => {
                //TODO
            }
            ZkSyncOp::WithdrawNFT(op) => {
                let token = self.get_token_by_id(storage, op.tx.token).await?;
                logs.push(
                    self.erc_transfer(
                        token,
                        op.tx.from,
                        H160::zero(),
                        BigUint::default(),
                        common_data,
                        0u8.into(),
                        storage,
                    )
                    .await?,
                );
            }
            ZkSyncOp::Swap(op) => {
                let token1 = self
                    .get_token_by_id(storage, op.tx.orders.0.token_buy)
                    .await?;
                let token2 = self
                    .get_token_by_id(storage, op.tx.orders.0.token_sell)
                    .await?;
                let address1 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.orders.0.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .expect("Can`t find account in storage");
                let address2 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.orders.1.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .expect("Can`t find account in storage");
                logs.push(
                    self.erc_transfer(
                        token1,
                        address1,
                        op.tx.orders.1.recipient_address,
                        op.tx.amounts.0,
                        common_data.clone(),
                        0u8.into(),
                        storage,
                    )
                    .await?,
                );
                logs.push(
                    self.erc_transfer(
                        token2,
                        address2,
                        op.tx.orders.0.recipient_address,
                        op.tx.amounts.1,
                        common_data.clone(),
                        1u8.into(),
                        storage,
                    )
                    .await?,
                );
            }
            ZkSyncOp::Deposit(op) => {
                let token = self.get_token_by_id(storage, op.priority_op.token).await?;
                logs.push(
                    self.erc_transfer(
                        token,
                        H160::zero(),
                        op.priority_op.to,
                        op.priority_op.amount,
                        common_data,
                        0u8.into(),
                        storage,
                    )
                    .await?,
                );
            }
            ZkSyncOp::FullExit(op) => {
                let token = self.get_token_by_id(storage, op.priority_op.token).await?;
                let account = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.priority_op.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .expect("Can`t find account in storage");
                logs.push(
                    self.erc_transfer(
                        token,
                        account,
                        H160::zero(),
                        op.withdraw_amount.unwrap_or_default().0,
                        common_data,
                        0u8.into(),
                        storage,
                    )
                    .await?,
                );
            }
            _ => {}
        };
        Ok(logs)
    }

    async fn get_token_by_id(
        &self,
        storage: &mut StorageProcessor<'_>,
        id: TokenId,
    ) -> jsonrpc_core::Result<Token> {
        Ok(self
            .tokens
            .get_token(storage, id)
            .await
            .map_err(|_| Error::internal_error())?
            .expect("Can't find token in storage"))
    }

    async fn get_nft_by_id(
        &self,
        storage: &mut StorageProcessor<'_>,
        id: TokenId,
    ) -> jsonrpc_core::Result<NFT> {
        Ok(self
            .tokens
            .get_nft_by_id(storage, id)
            .await
            .map_err(|_| Error::internal_error())?
            .expect("Can't find token in storage"))
    }

    fn zksync_op_log_index(tx: &ZkSyncOp) -> U256 {
        // For ChangePubKey there is no erc20/erc751 transfer, so zksync log is the first one,
        // for swaps there is two erc20/erc751 transfer, for other types that produce zksync log
        // there is only one. It doesn't matter what it returns for Noop and Close.
        if matches!(
            tx,
            ZkSyncOp::ChangePubKeyOffchain(_) | ZkSyncOp::MintNFTOp(_)
        ) {
            0u8.into()
        } else if matches!(tx, ZkSyncOp::Swap(_)) {
            2u8.into()
        } else {
            1u8.into()
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn erc_transfer(
        &self,
        token: Token,
        from: H160,
        to: H160,
        amount: BigUint,
        common_data: CommonLogData,
        transaction_log_index: U256,
        storage: &mut StorageProcessor<'_>,
    ) -> jsonrpc_core::Result<Log> {
        let (contract_address, amount_or_id) = if !token.is_nft {
            (token.address, u256_from_biguint(amount)?)
        } else {
            let nft = self
                .tokens
                .get_nft_by_id(storage, token.id)
                .await
                .map_err(|_| Error::internal_error())?
                .expect("Can't find token in storage");
            (nft.creator_address, nft.serial_id.into())
        };
        let data = Self::erc_transfer_data(from, to, amount_or_id);
        Ok(log(
            contract_address,
            self.topic_by_event(Event::ERCTransfer).unwrap(),
            data,
            common_data,
            transaction_log_index,
        ))
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
        creator_id: U256,
        creator_address: H160,
        content_hash: H256,
        recipient: H160,
        fee: U256,
        fee_token: H160,
    ) -> Bytes {
        let bytes = encode(&[
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
        token: H160,
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
            AbiToken::Address(token),
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
