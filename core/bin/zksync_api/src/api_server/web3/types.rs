//! Web3 API types definitions.
//!
//! Most of the types are re-exported from the `web3` crate, but some of them maybe extended with
//! new variants (enums) or optional fields (structures).
//!
//! These "extensions" are required to provide more zkSync-specific information while remaining Web3-compilant.

// Built-in uses
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;
// External uses
use itertools::unfold;
use jsonrpc_core::Error;
use num::BigUint;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use tiny_keccak::keccak256;
pub use web3::types::{
    Address, Block, Bytes, Log, Transaction, TransactionReceipt, H160, H2048, H256, H64, U256, U64,
};
// Workspace uses
use zksync_storage::{
    chain::operations_ext::records::{Web3TxData, Web3TxReceipt},
    StorageProcessor,
};
use zksync_types::{Token, TokenId, ZkSyncOp, NFT};
// Local uses
use super::converter::{log, u256_from_biguint};
use crate::utils::token_db_cache::TokenDBCache;

/// Block Number
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BlockNumber {
    /// Last block that was committed on L1.
    Committed,
    /// Last block that was finalized on L1.
    Finalized,
    /// Latest block (may be the block that is currently open).
    Latest,
    /// Earliest block (genesis)
    Earliest,
    /// Alias for `BlockNumber::Latest`.
    Pending,
    /// Block by number from canon chain
    Number(U64),
}

impl Serialize for BlockNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            BlockNumber::Number(ref x) => serializer.serialize_str(&format!("0x{:x}", x)),
            BlockNumber::Committed => serializer.serialize_str("committed"),
            BlockNumber::Finalized => serializer.serialize_str("finalized"),
            BlockNumber::Latest => serializer.serialize_str("latest"),
            BlockNumber::Earliest => serializer.serialize_str("earliest"),
            BlockNumber::Pending => serializer.serialize_str("pending"),
        }
    }
}

impl<'de> Deserialize<'de> for BlockNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = BlockNumber;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("A block number or one of the supported aliases")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let result = match value {
                    "committed" => BlockNumber::Committed,
                    "finalized" => BlockNumber::Finalized,
                    "latest" => BlockNumber::Latest,
                    "earliest" => BlockNumber::Earliest,
                    "pending" => BlockNumber::Pending,
                    num => {
                        let number =
                            U64::deserialize(de::value::BorrowedStrDeserializer::new(num))?;
                        BlockNumber::Number(number)
                    }
                };

                Ok(result)
            }
        }
        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Clone)]
pub struct TxData {
    pub block_hash: Option<H256>,
    pub block_number: Option<u32>,
    pub block_index: Option<u32>,
    pub from: H160,
    pub to: Option<H160>,
    pub nonce: u32,
    pub tx_hash: H256,
}

impl From<Web3TxData> for TxData {
    fn from(tx: Web3TxData) -> TxData {
        TxData {
            block_hash: tx.block_hash.map(|h| H256::from_slice(&h)),
            block_number: tx.block_number.map(|n| n as u32),
            block_index: tx.block_index.map(|i| i as u32),
            from: H160::from_slice(&tx.from_account),
            to: tx.to_account.map(|to| H160::from_slice(&to)),
            nonce: tx.nonce as u32,
            tx_hash: H256::from_slice(&tx.tx_hash),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockInfo {
    BlockWithHashes(Block<H256>),
    BlockWithTxs(Block<Transaction>),
}

impl BlockInfo {
    fn new_block<T>(
        hash: H256,
        parent_hash: H256,
        block_number: zksync_types::BlockNumber,
        timestamp: u64,
        transactions: Vec<T>,
    ) -> Block<T> {
        Block {
            hash: Some(hash),
            parent_hash,
            uncles_hash: H256::zero(),
            author: H160::zero(),
            state_root: hash,
            transactions_root: hash,
            receipts_root: hash,
            number: Some(block_number.0.into()),
            gas_used: 0.into(),
            gas_limit: 50000.into(),
            extra_data: Vec::new().into(),
            logs_bloom: None,
            timestamp: timestamp.into(),
            difficulty: 0.into(),
            total_difficulty: Some(0.into()),
            seal_fields: Vec::new(),
            uncles: Vec::new(),
            transactions,
            size: None,
            mix_hash: Some(H256::zero()),
            nonce: Some(H64::zero()),
        }
    }

    pub fn new_with_hashes(
        hash: H256,
        parent_hash: H256,
        block_number: zksync_types::BlockNumber,
        timestamp: u64,
        transactions: Vec<H256>,
    ) -> Self {
        Self::BlockWithHashes(Self::new_block(
            hash,
            parent_hash,
            block_number,
            timestamp,
            transactions,
        ))
    }

    pub fn new_with_txs(
        hash: H256,
        parent_hash: H256,
        block_number: zksync_types::BlockNumber,
        timestamp: u64,
        transactions: Vec<Transaction>,
    ) -> Self {
        Self::BlockWithTxs(Self::new_block(
            hash,
            parent_hash,
            block_number,
            timestamp,
            transactions,
        ))
    }
}

/// Either value or array of values.
#[derive(Default, Debug, PartialEq, Clone)]
pub struct ValueOrArray<T>(pub Vec<T>);

impl<'de, T: fmt::Debug + Deserialize<'de>> ::serde::Deserialize<'de> for ValueOrArray<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        struct Visitor<T>(PhantomData<T>);

        impl<'de, T: fmt::Debug + Deserialize<'de>> de::Visitor<'de> for Visitor<T> {
            type Value = ValueOrArray<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("Expected value or sequence")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use serde::de::IntoDeserializer;

                Deserialize::deserialize(value.into_deserializer())
                    .map(|value| ValueOrArray(vec![value]))
            }

            fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
            where
                S: de::SeqAccess<'de>,
            {
                unfold(visitor, |vis| vis.next_element().transpose())
                    .collect::<Result<_, _>>()
                    .map(ValueOrArray)
            }
        }

        deserializer.deserialize_any(Visitor(PhantomData))
    }
}

impl<T> Serialize for ValueOrArray<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0.len() {
            0 => serializer.serialize_none(),
            1 => Serialize::serialize(&self.0[0], serializer),
            _ => Serialize::serialize(&self.0, serializer),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Event {
    ZkSyncTransfer,
    ZkSyncWithdraw,
    ZkSyncForcedExit,
    ZkSyncChangePubKey,
    ZkSyncDeposit,
    ZkSyncFullExit,
    ZkSyncMintNFT,
    ZkSyncWithdrawNFT,
    ZkSyncSwap,
    ERCTransfer, // erc20 and erc721 transfers have same topics
}

#[derive(Debug, Clone)]
pub struct LogsHelper {
    topic_by_event: HashMap<Event, H256>,
    event_by_topic: HashMap<H256, Event>,
    tokens: TokenDBCache,
    pub zksync_proxy_address: H160,
}

#[derive(Debug, Clone)]
pub struct CommonLogData {
    pub block_hash: Option<H256>,
    pub block_number: Option<U64>,
    pub transaction_hash: H256,
    pub transaction_index: Option<U64>,
}

impl LogsHelper {
    pub fn new() -> Self {
        let data = vec![
            (Event::ZkSyncTransfer, "ZkSyncTransfer(address,address,address,uint256,uint256)"),
            (Event::ZkSyncWithdraw, "ZkSyncWithdraw(address,address,address,uint256,uint256)"),
            (Event::ZkSyncForcedExit, "ZkSyncForcedExit(address,address,address,uint256)"),
            (Event::ZkSyncChangePubKey, "ZkSyncChangePubKey(address,address,address,uint256)"),
            (Event::ZkSyncDeposit, "ZkSyncDeposit(address,address,address,uint256)"),
            (Event::ZkSyncFullExit, "ZkSyncFullExit(address,address)"),
            (Event::ZkSyncMintNFT, "ZkSyncMintNFT(uint32,address,address,bytes32,address,uint256)"),
            (Event::ZkSyncWithdrawNFT, "ZkSyncWithdrawNFT(address,address,bytes32,uint32,address,uint256)"),
            (Event::ZkSyncSwap, "ZkSyncSwap(address,address,address,address,address,address,uint256,uint256,uint256)"),
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
                    op.tx.recipient,
                    op.tx.content_hash,
                    fee_token.address,
                    u256_from_biguint(op.tx.fee)?,
                );
                Some((Event::ZkSyncMintNFT, data))
            }
            ZkSyncOp::WithdrawNFT(op) => {
                let fee_token = self.get_token_by_id(storage, op.tx.fee_token).await?;
                let nft = self.get_nft_by_id(storage, op.tx.token).await?;
                let data = Self::zksync_withdraw_nft_data(
                    nft.creator_address,
                    op.tx.to,
                    nft.content_hash,
                    op.tx.token.0.into(),
                    fee_token.address,
                    u256_from_biguint(op.tx.fee)?,
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
                let data = Self::zksync_swap_data(
                    op.tx.submitter_address,
                    address1,
                    address2,
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
                    op.priority_op.to,
                    op.priority_op.to,
                    token.address,
                    u256_from_biguint(op.priority_op.amount)?,
                );
                Some((Event::ZkSyncDeposit, data))
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
                let data = Self::zksync_full_exit_data(account, token.address);
                Some((Event::ZkSyncFullExit, data))
            }
            _ => None,
        };
        let log = log_data.map(|(event, data)| {
            log(
                self.zksync_proxy_address,
                *self.topic_by_event.get(&event).unwrap(),
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
                //let token = self.get_token_by_id(storage, tx.).await?;
                //logs.push(self.erc_transfer(token, tx.from, H160::zero(), BigUint::default(), common_data, 0u8.into(), storage).await?);
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
                let from1 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.orders.0.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .expect("Can`t find account in storage");
                let from2 = storage
                    .chain()
                    .account_schema()
                    .account_address_by_id(op.tx.orders.1.account_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .expect("Can`t find account in storage");
                logs.push(
                    self.erc_transfer(
                        token1,
                        from1,
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
                        from2,
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
        if matches!(tx, ZkSyncOp::ChangePubKeyOffchain(_)) {
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
            *self.topic_by_event.get(&Event::ERCTransfer).unwrap(),
            data,
            common_data,
            transaction_log_index,
        ))
    }

    fn append_bytes(value: &[u8]) -> Vec<u8> {
        let mut value = value.to_vec();
        let mut result = Vec::new();
        result.resize(32 - value.len(), 0);
        result.append(&mut value);
        result
    }

    fn u256_to_bytes(value: U256) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.resize(32, 0);
        value.to_big_endian(&mut bytes);
        bytes
    }

    fn erc_transfer_data(from: H160, to: H160, amount_or_id: U256) -> Bytes {
        let mut bytes = Vec::new();

        bytes.append(&mut Self::append_bytes(from.as_bytes()));
        bytes.append(&mut Self::append_bytes(to.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(amount_or_id)));
        bytes.into()
    }

    fn zksync_transfer_data(from: H160, to: H160, token: H160, amount: U256, fee: U256) -> Bytes {
        let mut bytes = Vec::new();

        bytes.append(&mut Self::append_bytes(from.as_bytes()));
        bytes.append(&mut Self::append_bytes(to.as_bytes()));
        bytes.append(&mut Self::append_bytes(token.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(amount)));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(fee)));
        bytes.into()
    }

    fn zksync_withdraw_data(from: H160, to: H160, token: H160, amount: U256, fee: U256) -> Bytes {
        Self::zksync_transfer_data(from, to, token, amount, fee)
    }

    fn zksync_forced_exit_data(initiator: H160, target: H160, token: H160, fee: U256) -> Bytes {
        let mut bytes = Vec::new();

        bytes.append(&mut Self::append_bytes(initiator.as_bytes()));
        bytes.append(&mut Self::append_bytes(target.as_bytes()));
        bytes.append(&mut Self::append_bytes(token.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(fee)));
        bytes.into()
    }

    fn zksync_change_pub_key_data(
        account: H160,
        new_pub_key_hash: [u8; 20],
        token: H160,
        fee: U256,
    ) -> Bytes {
        let mut bytes = Vec::new();

        bytes.append(&mut Self::append_bytes(account.as_bytes()));
        bytes.append(&mut Self::append_bytes(&new_pub_key_hash));
        bytes.append(&mut Self::append_bytes(token.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(fee)));
        bytes.into()
    }

    fn zksync_mint_nft_data(
        creator_id: U256,
        creator_address: H160,
        recipient: H160,
        content_hash: H256,
        fee_token: H160,
        fee: U256,
    ) -> Bytes {
        let mut bytes = Vec::new();

        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(creator_id)));
        bytes.append(&mut Self::append_bytes(creator_address.as_bytes()));
        bytes.append(&mut Self::append_bytes(recipient.as_bytes()));
        bytes.append(&mut Self::append_bytes(content_hash.as_bytes()));
        bytes.append(&mut Self::append_bytes(fee_token.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(fee)));
        bytes.into()
    }

    fn zksync_withdraw_nft_data(
        creator_address: H160,
        recipient: H160,
        content_hash: H256,
        token_id: U256,
        fee_token: H160,
        fee: U256,
    ) -> Bytes {
        let mut bytes = Vec::new();
        bytes.append(&mut Self::append_bytes(creator_address.as_bytes()));
        bytes.append(&mut Self::append_bytes(recipient.as_bytes()));
        bytes.append(&mut Self::append_bytes(content_hash.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(token_id)));
        bytes.append(&mut Self::append_bytes(fee_token.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(fee)));
        bytes.into()
    }

    #[allow(clippy::too_many_arguments)]
    fn zksync_swap_data(
        initiator: H160,
        account1: H160,
        account2: H160,
        fee_token: H160,
        token1: H160,
        token2: H160,
        fee: U256,
        amount1: U256,
        amount2: U256,
    ) -> Bytes {
        let mut bytes = Vec::new();
        bytes.append(&mut Self::append_bytes(initiator.as_bytes()));
        bytes.append(&mut Self::append_bytes(account1.as_bytes()));
        bytes.append(&mut Self::append_bytes(account2.as_bytes()));
        bytes.append(&mut Self::append_bytes(fee_token.as_bytes()));
        bytes.append(&mut Self::append_bytes(token1.as_bytes()));
        bytes.append(&mut Self::append_bytes(token2.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(fee)));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(amount1)));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(amount2)));
        bytes.into()
    }

    fn zksync_deposit_data(to: H160, from: H160, token: H160, amount: U256) -> Bytes {
        let mut bytes = Vec::new();
        bytes.append(&mut Self::append_bytes(to.as_bytes()));
        bytes.append(&mut Self::append_bytes(from.as_bytes()));
        bytes.append(&mut Self::append_bytes(token.as_bytes()));
        bytes.append(&mut Self::append_bytes(&Self::u256_to_bytes(amount)));
        bytes.into()
    }

    fn zksync_full_exit_data(account: H160, token: H160) -> Bytes {
        let mut bytes = Vec::new();
        bytes.append(&mut Self::append_bytes(account.as_bytes()));
        bytes.append(&mut Self::append_bytes(token.as_bytes()));
        bytes.into()
    }
}
