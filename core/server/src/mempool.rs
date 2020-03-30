//! Mempool is simple in memory buffer for transactions.
//!
//! Its role is to:
//! 1) Accept transactions from api, check signatures and basic nonce correctness(nonce not too small).
//! To do nonce correctness check mempool stores mapping `AccountAddress -> Nonce`, this mapping is updated
//! when new block is committed.
//! 2) When polled return vector of the transactions in the queue.
//!
//! Mempool is not persisted on disc, all transactions will be lost on node shutdown.
//!
//! Communication channel with other actors:
//! Mempool does not push information to other actors, only accepts requests. (see `MempoolRequest`)
//!
//! Communication with db:
//! on restart mempool restores nonces of the accounts that are stored in the account tree.

use crate::eth_watch::EthWatchRequest;
use failure::{format_err, Fail};
use futures::channel::{mpsc, oneshot};
use futures::{SinkExt, StreamExt};
use models::node::tx::TxEthSignature;
use models::node::{
    AccountId, AccountUpdate, AccountUpdates, FranklinTx, Nonce, PriorityOp, TokenId, TransferOp,
    TransferToNewOp,
};
use models::params::max_block_chunk_size;
use std::collections::{HashMap, VecDeque};
use storage::ConnectionPool;
use tokio::runtime::Runtime;
use web3::types::Address;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Fail)]
pub enum TxAddError {
    #[fail(display = "Tx nonce is too low.")]
    NonceMismatch,
    #[fail(display = "Tx is incorrect")]
    IncorrectTx,
    #[fail(display = "MissingEthSignature")]
    MissingEthSignature,
    #[fail(display = "Eth signature is incorrect")]
    IncorrectEthSignature,
    #[fail(display = "Change pubkey tx is not authorized onchain")]
    ChangePkNotAuthorized,
    #[fail(display = "Internal error")]
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct ProposedBlock {
    pub priority_ops: Vec<PriorityOp>,
    pub txs: Vec<FranklinTx>,
}

impl ProposedBlock {
    pub fn is_empty(&self) -> bool {
        self.priority_ops.is_empty() && self.txs.is_empty()
    }
}

pub struct GetBlockRequest {
    pub last_priority_op_number: u64,
    pub response_sender: oneshot::Sender<ProposedBlock>,
}

pub enum MempoolRequest {
    /// Add new transaction to mempool, check signature and correctness
    /// oneshot is used to receive tx add result.
    NewTx(
        Box<FranklinTx>,
        Box<Option<TxEthSignature>>,
        oneshot::Sender<Result<(), TxAddError>>,
    ),
    /// When block is committed, nonces of the account tree should be updated too.
    UpdateNonces(AccountUpdates),
    /// Get transactions from the mempool.
    GetBlock(GetBlockRequest),
}

struct MempoolState {
    // account and last committed nonce
    account_nonces: HashMap<Address, Nonce>,
    account_ids: HashMap<AccountId, Address>,
    ready_txs: VecDeque<FranklinTx>,
}

impl MempoolState {
    fn chunks_for_tx(&self, tx: &FranklinTx) -> usize {
        match tx {
            FranklinTx::Transfer(tx) => {
                if self.account_nonces.contains_key(&tx.to) {
                    TransferOp::CHUNKS
                } else {
                    TransferToNewOp::CHUNKS
                }
            }
            _ => tx.min_chunks(),
        }
    }

    fn restore_from_db(db_pool: &ConnectionPool) -> Self {
        let storage = db_pool.access_storage().expect("mempool db restore");
        let (_, accounts) = storage
            .chain()
            .state_schema()
            .load_committed_state(None)
            .expect("mempool account state load");

        let mut account_ids = HashMap::new();
        let mut account_nonces = HashMap::new();

        for (id, account) in accounts {
            account_ids.insert(id, account.address.clone());
            account_nonces.insert(account.address, account.nonce);
        }

        Self {
            account_nonces,
            account_ids,
            ready_txs: VecDeque::new(),
        }
    }

    fn nonce(&self, address: &Address) -> Nonce {
        *self.account_nonces.get(address).unwrap_or(&0)
    }

    fn add_tx(&mut self, tx: FranklinTx) -> Result<(), TxAddError> {
        if !tx.check_correctness() {
            return Err(TxAddError::IncorrectTx);
        }

        if tx.nonce() >= self.nonce(&tx.account()) {
            self.ready_txs.push_back(tx);
            Ok(())
        } else {
            Err(TxAddError::NonceMismatch)
        }
    }
}

struct Mempool {
    mempool_state: MempoolState,
    requests: mpsc::Receiver<MempoolRequest>,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    token_cache: TokenCache,
}

impl Mempool {
    async fn add_tx(
        &mut self,
        tx: FranklinTx,
        signature: Option<TxEthSignature>,
    ) -> Result<(), TxAddError> {
        if let FranklinTx::ChangePubKey(change_pk) = &tx {
            if change_pk.eth_signature.is_none() {
                let eth_watch_resp = oneshot::channel();
                self.eth_watch_req
                    .clone()
                    .send(EthWatchRequest::IsPubkeyChangeAuthorized {
                        address: change_pk.account,
                        nonce: change_pk.nonce,
                        pubkey_hash: change_pk.new_pk_hash.clone(),
                        resp: eth_watch_resp.0,
                    })
                    .await
                    .expect("ETH watch req receiver dropped");

                let is_authorized = eth_watch_resp.1.await.expect("Err response from eth watch");
                if !is_authorized {
                    return Err(TxAddError::ChangePkNotAuthorized);
                }
            }
        }

        let message_to_sign = tx
            .get_tx_info_message_to_sign(&mut |token_id| {
                self.token_cache
                    .token_symbol_from_id(token_id)?
                    .ok_or_else(|| format_err!("No symbol for TokenId {}", token_id))
            })
            .or(Err(TxAddError::IncorrectTx))?;

        if let Some(message_to_sign) = message_to_sign {
            let tx_eth_signature = signature.ok_or(TxAddError::MissingEthSignature)?;

            match tx_eth_signature {
                TxEthSignature::EthereumSignature(packed_signature) => {
                    let signer_account = packed_signature
                        .signature_recover_signer(message_to_sign.as_bytes())
                        .or(Err(TxAddError::IncorrectEthSignature))?;

                    if signer_account != tx.account() {
                        return Err(TxAddError::IncorrectEthSignature);
                    }
                }
                TxEthSignature::EIP1271Signature(signature) => {
                    let eth_watch_resp = oneshot::channel();
                    self.eth_watch_req
                        .clone()
                        .send(EthWatchRequest::CheckEIP1271Signature {
                            address: tx.account(),
                            data: message_to_sign.as_bytes().to_vec(),
                            signature,
                            resp: eth_watch_resp.0,
                        })
                        .await
                        .expect("ETH watch req receiver dropped");

                    if !eth_watch_resp.1.await.expect("Err response from eth watch") {
                        return Err(TxAddError::IncorrectEthSignature);
                    }
                }
            };
        };

        self.mempool_state.add_tx(tx)
    }

    async fn run(mut self) {
        while let Some(request) = self.requests.next().await {
            match request {
                MempoolRequest::NewTx(tx, signature, resp) => {
                    let tx_add_result = self.add_tx(*tx, *signature).await;
                    resp.send(tx_add_result).unwrap_or_default();
                }
                MempoolRequest::GetBlock(block) => {
                    block
                        .response_sender
                        .send(self.propose_new_block(block.last_priority_op_number).await)
                        .expect("mempool proposed block response send failed");
                }
                MempoolRequest::UpdateNonces(updates) => {
                    for (id, update) in updates {
                        match update {
                            AccountUpdate::Create { address, nonce } => {
                                self.mempool_state.account_ids.insert(id, address.clone());
                                self.mempool_state.account_nonces.insert(address, nonce);
                            }
                            AccountUpdate::Delete { address, .. } => {
                                self.mempool_state.account_ids.remove(&id);
                                self.mempool_state.account_nonces.remove(&address);
                            }
                            AccountUpdate::UpdateBalance { new_nonce, .. } => {
                                if let Some(address) = self.mempool_state.account_ids.get(&id) {
                                    if let Some(nonce) =
                                        self.mempool_state.account_nonces.get_mut(address)
                                    {
                                        *nonce = new_nonce;
                                    }
                                }
                            }
                            AccountUpdate::ChangePubKeyHash { new_nonce, .. } => {
                                if let Some(address) = self.mempool_state.account_ids.get(&id) {
                                    if let Some(nonce) =
                                        self.mempool_state.account_nonces.get_mut(address)
                                    {
                                        *nonce = new_nonce;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn propose_new_block(&mut self, current_unprocessed_priority_op: u64) -> ProposedBlock {
        let (chunks_left, priority_ops) = self
            .select_priority_ops(current_unprocessed_priority_op)
            .await;
        let (_chunks_left, txs) = self.prepare_tx_for_block(chunks_left);
        trace!("Proposed priority ops for block: {:#?}", priority_ops);
        trace!("Proposed txs for block: {:#?}", txs);
        ProposedBlock { priority_ops, txs }
    }

    /// Returns: chunks left from max amount of chunks, ops selected
    async fn select_priority_ops(
        &self,
        current_unprocessed_priority_op: u64,
    ) -> (usize, Vec<PriorityOp>) {
        let eth_watch_resp = oneshot::channel();
        self.eth_watch_req
            .clone()
            .send(EthWatchRequest::GetPriorityQueueOps {
                op_start_id: current_unprocessed_priority_op,
                max_chunks: max_block_chunk_size(),
                resp: eth_watch_resp.0,
            })
            .await
            .expect("ETH watch req receiver dropped");

        let priority_ops = eth_watch_resp.1.await.expect("Err response from eth watch");

        (
            max_block_chunk_size()
                - priority_ops
                    .iter()
                    .map(|op| op.data.chunks())
                    .sum::<usize>(),
            priority_ops,
        )
    }

    fn prepare_tx_for_block(&mut self, mut chunks_left: usize) -> (usize, Vec<FranklinTx>) {
        let mut txs_for_commit = Vec::new();
        let mut tx_for_reinsert = None;

        while let Some(tx) = self.mempool_state.ready_txs.pop_front() {
            let chunks_for_tx = self.mempool_state.chunks_for_tx(&tx);
            if chunks_left >= chunks_for_tx {
                txs_for_commit.push(tx);
                chunks_left -= chunks_for_tx;
            } else {
                tx_for_reinsert = Some(tx);
                break;
            }
        }

        if let Some(tx) = tx_for_reinsert {
            self.mempool_state.ready_txs.push_front(tx);
        }

        (chunks_left, txs_for_commit)
    }
}

struct TokenCache {
    db_pool: ConnectionPool,
    ids_to_symbols: HashMap<TokenId, String>,
}

impl TokenCache {
    pub fn new(db_pool: ConnectionPool) -> Self {
        Self {
            db_pool,
            ids_to_symbols: HashMap::new(),
        }
    }

    pub fn token_symbol_from_id(
        &mut self,
        token_id: TokenId,
    ) -> Result<Option<String>, failure::Error> {
        match self.ids_to_symbols.get(&token_id).cloned() {
            Some(token_symbol) => Ok(Some(token_symbol)),
            None => {
                match self
                    .db_pool
                    .access_storage_fragile()
                    .map_err(|e| format_err!("Failed to access storage: {}", e))?
                    .tokens_schema()
                    .load_tokens()
                    .map_err(|e| format_err!("Tokens load failed: {}", e))?
                    .get(&token_id)
                    .cloned()
                {
                    Some(token_info) => Ok(Some(
                        self.ids_to_symbols
                            .entry(token_id)
                            .or_insert(token_info.symbol)
                            .clone(),
                    )),
                    None => Ok(None),
                }
            }
        }
    }
}

pub fn run_mempool_task(
    db_pool: ConnectionPool,
    requests: mpsc::Receiver<MempoolRequest>,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    runtime: &Runtime,
) {
    let mempool_state = MempoolState::restore_from_db(&db_pool);

    let token_cache = TokenCache::new(db_pool);

    let mempool = Mempool {
        mempool_state,
        requests,
        eth_watch_req,
        token_cache,
    };
    runtime.spawn(mempool.run());
}
