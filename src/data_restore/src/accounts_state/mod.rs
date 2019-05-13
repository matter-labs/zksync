use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use ff::{Field, PrimeField, PrimeFieldRepr};
use web3::futures::Future;
use web3::types::{Address, FilterBuilder, H256, U256, BlockNumber};
use ethabi::Contract;
use sapling_crypto::jubjub::{edwards, Unknown};

use bigdecimal::{Num, BigDecimal, Zero};

use plasma::models::{Account, AccountTree, AccountId};
use plasma::models::{DepositTx, TransferTx, Engine, Fr, ExitTx, TxSignature};
use plasma::models::params;

use helpers::*;
use franklin_transaction::{FranklinTransactionType,FranklinTransaction};

#[derive(Debug, Clone)]
pub struct FullExitTransactionsBlock {
    pub batch_number: u32,
    pub block_number: u32,
    pub exits: Vec<ExitTx>,
}

#[derive(Debug, Clone)]
pub struct DepositTransactionsBlock {
    pub batch_number: u32,
    pub block_number: u32,
    pub deposits: Vec<DepositTx>,
}

#[derive(Debug, Clone)]
pub struct TransferTransactionsBlock {
    pub block_number: u32,
    pub transfers: Vec<TransferTx>,
}

pub struct FranklinAccountsStates {
    pub http_endpoint_string: String,
    pub franklin_abi: ABI,
    pub franklin_contract: Contract,
    pub franklin_contract_address: Address,
    pub accounts_tree: AccountTree,
}

impl FranklinAccountsStates {
    pub fn new(network: InfuraEndpoint) -> Self {
        let http_infura_endpoint_str = match network {
            InfuraEndpoint::Mainnet => INFURA_MAINNET_ENDPOINT,
            InfuraEndpoint::Rinkeby => INFURA_RINKEBY_ENDPOINT,
        };
        let http_infura_endpoint_string = String::from(http_infura_endpoint_str);
        let address: Address = match network {
            InfuraEndpoint::Mainnet => FRANKLIN_MAINNET_ADDRESS,
            InfuraEndpoint::Rinkeby => FRANKLIN_RINKEBY_ADDRESS,
        }.parse().unwrap();
        let abi: ABI = match network {
            InfuraEndpoint::Mainnet => PLASMA_MAINNET_ABI,
            InfuraEndpoint::Rinkeby => PLASMA_RINKEBY_ABI,
        };
        let contract = ethabi::Contract::load(abi.0).unwrap();

        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let tree = AccountTree::new(tree_depth);
        
        let this = Self {
            http_endpoint_string: http_infura_endpoint_string,
            franklin_abi: abi,
            franklin_contract: contract,
            franklin_contract_address: address,
            accounts_tree: tree,
        };
        this
    }

    pub fn update_accounts_states_from_transaction(&mut self, transaction: &FranklinTransaction) -> Result<(), DataRestoreError> {
        let tx_type = transaction.franklin_transaction_type;
        match tx_type {
            FranklinTransactionType::Deposit => {
                let _ = self.update_accounts_states_from_deposit_transaction(transaction);
                Ok(())
            },
            FranklinTransactionType::FullExit => {
                let _ = self.update_accounts_states_from_full_exit_transaction(transaction);
                Ok(())
            },
            FranklinTransactionType::Transfer => {
                let _ = self.update_accounts_states_from_transfer_transaction(transaction);
                Ok(())
            },
            _ => return Err(DataRestoreError::WrongType)
        }
    }

    pub fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.accounts_tree.items.iter().map(|a| (*a.0 as u32, a.1.clone()) ).collect()
    }

    pub fn root_hash(&self) -> Fr {
        self.accounts_tree.root_hash().clone()
    }

    pub fn get_account(&self, account_id: AccountId) -> Option<Account> {
        self.accounts_tree.items.get(&account_id).cloned()
    }

    fn update_accounts_states_from_transfer_transaction(&mut self, transaction: &FranklinTransaction) -> Result<(), DataRestoreError> {
        // println!("tx: {:?}", transaction.ethereum_transaction.hash);
        let transfer_txs_block = self.get_all_transactions_from_transfer_block(transaction).map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        for tx in transfer_txs_block.transfers {
            if let Some(mut from) = self.accounts_tree.items.get(&tx.from).cloned() {
                let mut transacted_amount = BigDecimal::zero();
                // println!("amount tx: {:?}", &tx.amount);
                transacted_amount += &tx.amount;
                transacted_amount += &tx.fee;

                if from.balance < transacted_amount {
                    return Err(DataRestoreError::WrongAmount)
                }
                
                let mut to = Account::default();
                if let Some(existing_to) = self.accounts_tree.items.get(&tx.to) {
                    to = existing_to.clone();
                }

                from.balance -= transacted_amount;

                from.nonce += 1;
                if tx.to != 0 {
                    to.balance += &tx.amount;
                }

                self.accounts_tree.insert(tx.from, from);
                self.accounts_tree.insert(tx.to, to);
            } else {
                return Err(DataRestoreError::NonexistentAccount)
            }
        }
        Ok(())
    }

    fn update_accounts_states_from_deposit_transaction(&mut self, transaction: &FranklinTransaction) -> Result<(), DataRestoreError> {
        let batch_number = self.get_batch_number_from_deposit(transaction);
        let block_number = self.get_block_number_from_deposit(transaction);
        let deposit_txs_block = self.get_all_transactions_from_deposit_batch(batch_number, block_number).map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        for tx in deposit_txs_block.deposits {
            let account = match self.accounts_tree.items.get(&tx.account) {
                None => {
                    let mut acc = Account::default();
                    let tx = tx.clone();
                    acc.public_key_x = tx.pub_x;
                    acc.public_key_y = tx.pub_y;
                    acc.balance = tx.amount;
                    acc
                },
                Some(result) => {
                    let mut acc = result.clone();
                    acc.balance += &tx.amount;
                    acc
                }
            };
            self.accounts_tree.insert(tx.account, account);
        }
        Ok(())
    }

    fn update_accounts_states_from_full_exit_transaction(&mut self, transaction: &FranklinTransaction) -> Result<(), DataRestoreError> {
        let batch_number = self.get_batch_number_from_full_exit(transaction);
        let block_number = self.get_block_number_from_full_exit(transaction);
        let exit_txs_block = self.get_all_transactions_from_full_exit_batch(batch_number, block_number).map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        for tx in exit_txs_block.exits {
            if let None = self.accounts_tree.items.get(&tx.account).cloned() {
                return Err(DataRestoreError::NonexistentAccount)
            }
            self.accounts_tree.delete(tx.account);
        }
        Ok(())
    }

    fn get_batch_number_from_deposit(&self, transaction: &FranklinTransaction) -> U256 {
        let batch_vec = transaction.commitment_data[0..32].to_vec();
        let batch_slice = batch_vec.as_slice();
        U256::from(batch_slice)
    }

    fn get_batch_number_from_full_exit(&self, transaction: &FranklinTransaction) -> U256 {
        let batch_vec = transaction.commitment_data[0..32].to_vec();
        let batch_slice = batch_vec.as_slice();
        U256::from(batch_slice)
    }

    fn get_block_number_from_deposit(&self, transaction: &FranklinTransaction) -> U256 {
        let block_vec = transaction.commitment_data[64..96].to_vec();
        let block_slice = block_vec.as_slice();
        U256::from(block_slice)
    }

    fn get_block_number_from_full_exit(&self, transaction: &FranklinTransaction) -> U256 {
        let block_vec = transaction.commitment_data[64..96].to_vec();
        let block_slice = block_vec.as_slice();
        U256::from(block_slice)
    }

    fn get_all_transactions_from_transfer_block(&self, transaction: &FranklinTransaction) -> Result<TransferTransactionsBlock, DataRestoreError> {
        let mut tx_data_vec = transaction.commitment_data.clone();
        let block_number = &transaction.commitment_data.clone()[0..32];
        let mut tx_data_len = tx_data_vec.len();
        tx_data_vec.truncate(tx_data_len-24);
        tx_data_vec.reverse();
        tx_data_len = tx_data_vec.len();
        tx_data_vec.truncate(tx_data_len-160);
        tx_data_vec.reverse();
        
        let (txs0l, txs0r) = tx_data_vec.split_at(36);
        let (txs1l, txs1r) = txs0l.split_at(18);
        let (txs2l, txs2r) = txs1l.split_at(9);
        let (txs3l, txs3r) = txs1r.split_at(9);
        let (txs4l, txs4r) = txs0r.split_at(18);
        let (txs5l, txs5r) = txs4l.split_at(9);
        let (txs6l, txs6r) = txs4r.split_at(9);

        let txs = vec![txs2l, txs2r, txs3l, txs3r, txs5l, txs5r, txs6l, txs6r];
        let mut transfers: Vec<TransferTx> = vec![];
        for tx in txs {
            if tx != [0, 0, 2, 0, 0, 0, 0, 0, 0] {
                let from = U256::from(&tx[0..3]);
                let to = U256::from(&tx[3..6]);
                let amount = U256::from(&tx[6..8]);
                let fee = U256::from(tx[8]);
                let transfer_tx = TransferTx {
                    from: from.as_u32(),
                    to: to.as_u32(),
                    amount: BigDecimal::from_str_radix(&format!("{}", amount), 10).unwrap(),
                    fee: BigDecimal::from_str_radix(&format!("{}", fee), 10).unwrap(),
                    nonce: 0,
                    good_until_block: 0,
                    signature: TxSignature::default(),
                    cached_pub_key: None,
                };
                transfers.push(transfer_tx);
            }
        }
        
        Ok(TransferTransactionsBlock {
            block_number: U256::from(block_number).as_u32(),
            transfers: transfers,
        })
    }

    fn get_all_transactions_from_deposit_batch(&self, batch_number: U256, block_number: U256) -> Result<DepositTransactionsBlock, DataRestoreError> {
        let (_eloop, transport) = web3::transports::Http::new(self.http_endpoint_string.as_str()).map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let deposit_event = self.franklin_contract.event("LogDepositRequest").unwrap().clone();
        let deposit_event_topic = deposit_event.signature();
        let deposits_filter = FilterBuilder::default()
            .address(vec![self.franklin_contract_address.clone()])
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .topics(
                Some(vec![deposit_event_topic]),
                Some(vec![H256::from(batch_number.clone())]),
                None,
                None,
            )
            .build();
        let mut deposit_events_filter_result = web3.eth().logs(deposits_filter).wait().map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let mut error_flag = false;
        deposit_events_filter_result.sort_by(|l, r| {
            let l_block = l.block_number.unwrap();
            let r_block = r.block_number.unwrap();

            if l_block > r_block {
                return Ordering::Greater;
            } else if l_block < r_block {
                return Ordering::Less;
            }

            let l_index = l.log_index.unwrap();
            let r_index = r.log_index.unwrap();
            if l_index > r_index {
                return Ordering::Greater;
            } else if l_index < r_index {
                return Ordering::Less;
            }
            error_flag = true;
            return Ordering::Equal; // Need to rewrite
        });
        if error_flag {
            return Err(DataRestoreError::Unknown("Logs can not have same indexes".to_string()));
        }
        let mut this_batch: HashMap<U256, (U256, U256)> = HashMap::new();

        for event in deposit_events_filter_result {
            let data_bytes: Vec<u8> = event.data.0;
            let account_id = U256::from(event.topics[2]);
            let public_key = U256::from(event.topics[3]);
            let deposit_amount = U256::from_big_endian(&data_bytes);
            let existing_record = this_batch.get(&account_id).map(|&v| v.clone());
            if let Some(record) = existing_record {
                let mut existing_balance = record.0;
                existing_balance = existing_balance + deposit_amount;
                this_batch.insert(account_id, (existing_balance, record.1));
            } else {
                this_batch.insert(account_id, (deposit_amount, public_key));
            }
            continue;
        }
        let mut all_deposits = vec![];
        for (k, v) in this_batch.iter() {
            let mut public_key_bytes = vec![0u8; 32];
            v.1.to_big_endian(& mut public_key_bytes);
            let x_sign = public_key_bytes[0] & 0x80 > 0;
            public_key_bytes[0] &= 0x7f;
            let mut fe_repr = Fr::zero().into_repr();
            fe_repr.read_be(public_key_bytes.as_slice()).expect("read public key point");
            let y = Fr::from_repr(fe_repr);
            if y.is_err() {
                return Err(DataRestoreError::WrongPubKey)
            }
            let public_key_point = edwards::Point::<Engine, Unknown>::get_for_y(y.unwrap(), x_sign, &params::JUBJUB_PARAMS);
            if public_key_point.is_none() {
                return Err(DataRestoreError::WrongPubKey)
            }

            let (pub_x, pub_y) = public_key_point.unwrap().into_xy();

            let tx: DepositTx = DepositTx{ 
                account: k.as_u32(),
                amount:  BigDecimal::from_str_radix(&format!("{}", v.0), 10).unwrap(),
                pub_x:   pub_x,
                pub_y:   pub_y,
            };
            all_deposits.push(tx);
        }
        let block = DepositTransactionsBlock {
            batch_number: batch_number.as_u32(),
            block_number: block_number.as_u32(),
            deposits: all_deposits,
        };
        Ok(block)
    }

    fn get_all_transactions_from_full_exit_batch(&self, batch_number: U256, block_number: U256) -> Result<FullExitTransactionsBlock, DataRestoreError> {
        let (_eloop, transport) = web3::transports::Http::new(self.http_endpoint_string.as_str()).map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let exit_event = self.franklin_contract.event("LogExitRequest").unwrap().clone();
        let exit_event_topic = exit_event.signature();
        let exits_filter = FilterBuilder::default()
            .address(vec![self.franklin_contract_address.clone()])
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .topics(
                Some(vec![exit_event_topic]),
                Some(vec![H256::from(batch_number.clone())]),
                None,
                None,
            )
            .build();
        let mut exit_events_filter_result = web3.eth().logs(exits_filter).wait().map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let mut error_flag = false;
        exit_events_filter_result.sort_by(|l, r| {
            let l_block = l.block_number.unwrap();
            let r_block = r.block_number.unwrap();

            if l_block > r_block {
                return Ordering::Greater;
            } else if l_block < r_block {
                return Ordering::Less;
            }

            let l_index = l.log_index.unwrap();
            let r_index = r.log_index.unwrap();
            if l_index > r_index {
                return Ordering::Greater;
            } else if l_index < r_index {
                return Ordering::Less;
            }
            error_flag = true;
            return Ordering::Equal; // Need to rewrite
        });
        if error_flag {
            return Err(DataRestoreError::Unknown("Logs can not have same indexes".to_string()));
        }
        let mut this_batch: HashSet<U256> = HashSet::new();

        for event in exit_events_filter_result {
            let account_id = U256::from(event.topics[2]);;
            let existing_record = this_batch.get(&account_id).map(|&v| v.clone());
            if let Some(_) = existing_record {
                return Err(DataRestoreError::DoubleExit)
            } else {
                this_batch.insert(account_id);
            }
            continue;
        }
        let mut all_exits = vec![];
        for k in this_batch.iter() {
            let tx: ExitTx = ExitTx {
                account: k.as_u32(),
                amount:  BigDecimal::zero(),
            };
            all_exits.push(tx);
        }
        let block = FullExitTransactionsBlock {
            batch_number: batch_number.as_u32(),
            block_number: block_number.as_u32(),
            exits: all_exits,
        };
        Ok(block)
    }
}