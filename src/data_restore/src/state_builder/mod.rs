use std::rc::Rc;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use ff::{Field, PrimeField, PrimeFieldRepr};
use web3::futures::{Future, Stream};
use web3::types::{Log, Address, FilterBuilder, H256, U256, BlockNumber};
use ethabi::Contract;
use sapling_crypto::jubjub::{edwards, Unknown};

use plasma::models::{Block, BlockData, DepositTx, Engine, Fr, ExitTx};
use plasma::models::params;
use plasma::models::circuit::{AccountTree, Account};

use blocks::{BlockType, LogBlockData};
use helpers;
use helpers::InfuraEndpoint;
use franklin_transaction::{FranklinTransactionType,FranklinTransaction};

type ABI = (&'static [u8], &'static str);
type ComAndVerBlocksVecs = (Vec<LogBlockData>, Vec<LogBlockData>);
type BlockNumber256 = U256;

pub const PLASMA_TEST_ABI: ABI = (
    include_bytes!("../../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.abi"),
    include_str!("../../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.bin"),
);

pub const PLASMA_PROD_ABI: ABI = (
    include_bytes!("../../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.abi"),
    include_str!("../../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.bin"),
);

#[derive(Debug, Clone)]
pub struct FullExitTransactionsBlock {
    pub batch_number: U256,
    pub exits: Vec<FullExitTransaction>,
}

#[derive(Debug, Copy, Clone)]
pub struct FullExitTransaction {
    pub account_id: u32,
}

#[derive(Debug, Clone)]
pub struct DepositTransactionsBlock {
    pub batch_number: U256,
    pub deposits: Vec<DepositTransaction>,
}

#[derive(Debug, Copy, Clone)]
pub struct DepositTransaction {
    pub account_id: u32,
    pub pub_x: Fr,
    pub pub_y: Fr,
    pub amount: U256,
}

#[derive(Debug, Copy, Clone)]
pub struct FranklinAccountState {
    pub batch_number: U256,
    pub block_number: U256,
    pub pub_x: Fr,
    pub pub_y: Fr,
    pub balance: U256,
    pub nonce: U256,
}

#[derive(Debug, Clone)]
pub struct FranklinAccount {
    pub account_id: u32,
    pub states: Vec<FranklinAccountState>
}

pub struct StatesBuilderFranklin {
    pub http_endpoint_string: String,
    pub franklin_abi: ABI,
    pub franklin_contract: Contract,
    pub franklin_contract_address: Address,
    pub accounts_tree: AccountTree,
    pub accounts_franklin: Vec<FranklinAccount>,
}

impl StatesBuilderFranklin {
    pub fn new(network: InfuraEndpoint) -> Self {
        let http_infura_endpoint_str = match network {
            InfuraEndpoint::Mainnet => "https://mainnet.infura.io/",
            InfuraEndpoint::Rinkeby => "https://rinkeby.infura.io/",
        };
        let http_infura_endpoint_string = String::from(http_infura_endpoint_str);
        let address: Address = match network {
            InfuraEndpoint::Mainnet => "fddb8167fef957f7cc72686094fac1d31be5ecfe",
            InfuraEndpoint::Rinkeby => "fddb8167fef957f7cc72686094fac1d31be5ecfe",
        }.parse().unwrap();
        let abi: ABI = match network {
            InfuraEndpoint::Mainnet => PLASMA_PROD_ABI,
            InfuraEndpoint::Rinkeby => PLASMA_TEST_ABI,
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
            accounts_franklin: vec![],
        };
        this
    }

    pub fn update_accounts_states_from_transaction(&mut self, transaction: &FranklinTransaction) -> Result<(), String> {
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
            _ => return Err(String::from("Wrong tx type"))
        }
    }

    fn update_accounts_states_from_deposit_transaction(&mut self, transaction: &FranklinTransaction) -> Result<(), String> {
        let batch_number = self.get_batch_number_from_deposit(transaction);
        let block_number = self.get_block_number_from_deposit(transaction);
        let deposit_txs_block = self.get_all_transactions_from_deposit_batch(batch_number);
        if deposit_txs_block.is_err() {
            return Err(String::from("No deposit txs in block"));
        }
        for deposit in deposit_txs_block.unwrap().deposits {
            let id = deposit.account_id;
            let accounts = self.accounts_franklin.clone();
            let index = accounts.iter().position(|x| x.account_id == id);
            // let mut account = all_accounts_iter.find(|&&x| x.account_id == id);
            match index {
                Some(i) => {
                    let accs = accounts.clone();
                    let last_state = accs[i].states.iter().max_by_key(|x| x.batch_number);
                    if last_state.is_none() {
                        return Err(String::from("Wrong deposit in list"));
                    }
                    let unwraped_last_state = last_state.unwrap();
                    let last_balance = unwraped_last_state.balance;
                    let last_nonce = unwraped_last_state.nonce;
                    let new_balance = last_balance + deposit.amount;
                    let new_nonce = last_nonce + 1;
                    let new_state = FranklinAccountState {
                        batch_number: batch_number,
                        block_number: block_number,
                        pub_x: deposit.pub_x,
                        pub_y: deposit.pub_y,
                        balance: new_balance,
                        nonce: new_nonce,
                    };
                    self.accounts_franklin[i].states.push(new_state);
                },
                None => {
                    let state = FranklinAccountState {
                        batch_number: batch_number,
                        block_number: block_number,
                        pub_x: deposit.pub_x,
                        pub_y: deposit.pub_y,
                        balance: deposit.amount,
                        nonce: U256::from(0),
                    };
                    let account = FranklinAccount {
                        account_id: id,
                        states: vec![state],
                    };
                    self.accounts_franklin.push(account);
                }
            }
        }
        Ok(())
    }

    fn update_accounts_states_from_full_exit_transaction(&mut self, transaction: &FranklinTransaction) -> Result<(), String> {
        let batch_number = self.get_batch_number_from_full_exit(transaction);
        let block_number = self.get_block_number_from_full_exit(transaction);
        let exit_txs_block = self.get_all_transactions_from_full_exit_batch(batch_number);
        if exit_txs_block.is_err() {
            return Err(String::from("No exit txs in block"));
        }
        for exit in exit_txs_block.unwrap().exits {
            let id = exit.account_id;
            let accounts = self.accounts_franklin.clone();
            let index = accounts.iter().position(|x| x.account_id == id);
            // let mut account = all_accounts_iter.find(|&&x| x.account_id == id);
            match index {
                Some(i) => {
                    let accs = accounts.clone();
                    let last_state = accs[i].states.iter().max_by_key(|x| x.batch_number);
                    if last_state.is_none() {
                        return Err(String::from("Wrong deposit in list"));
                    }
                    let unwraped_last_state = last_state.unwrap();
                    let pub_x = unwraped_last_state.pub_x;
                    let pub_y = unwraped_last_state.pub_y;
                    let last_nonce = unwraped_last_state.nonce;
                    let new_nonce = last_nonce + 1;
                    let new_state = FranklinAccountState {
                        batch_number: batch_number,
                        block_number: block_number,
                        pub_x: pub_x,
                        pub_y: pub_y,
                        balance: U256::zero(),
                        nonce: new_nonce,
                    };
                    self.accounts_franklin[i].states.push(new_state);
                },
                None => {
                    panic!("Cant find account for exit");
                }
            }
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

    fn get_all_transactions_from_deposit_batch(&self, batch_number: U256) -> Result<DepositTransactionsBlock, String> {
        let (_eloop, transport) = match web3::transports::Http::new(self.http_endpoint_string.as_str()) {
            Err(_) => return Err(String::from("Error creating web3 with this endpoint")),
            Ok(result) => result,
        };
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
        let deposit_events_filter_result = web3.eth().logs(deposits_filter).wait();
        if deposit_events_filter_result.is_err() {
            return Err(String::from("Cant find deposit event"))
        }
        let mut deposit_events = deposit_events_filter_result.unwrap();
        deposit_events.sort_by(|l, r| {
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
            panic!("Logs can not have same indexes");
        });
        let mut this_batch: HashMap<U256, (U256, U256)> = HashMap::new();

        for event in deposit_events {
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
                return Err(String::from("Wrong public key"))
            }
            let public_key_point = edwards::Point::<Engine, Unknown>::get_for_y(y.unwrap(), x_sign, &params::JUBJUB_PARAMS);
            if public_key_point.is_none() {
                return Err(String::from("Wrong public key"))
            }

            let (pub_x, pub_y) = public_key_point.unwrap().into_xy();

            let deposit: DepositTransaction = DepositTransaction{
                account_id: k.as_u32(),
                pub_x:      pub_x,
                pub_y:      pub_y,
                amount:     v.0,
            };
            all_deposits.push(deposit);
        }
        let block = DepositTransactionsBlock {
            batch_number: batch_number.clone(),
            deposits: all_deposits,
        };
        Ok(block)
    }

    fn get_all_transactions_from_full_exit_batch(&self, batch_number: U256) -> Result<FullExitTransactionsBlock, String> {
        let (_eloop, transport) = match web3::transports::Http::new(self.http_endpoint_string.as_str()) {
            Err(_) => return Err(String::from("Error creating web3 with this endpoint")),
            Ok(result) => result,
        };
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
        let exit_events_filter_result = web3.eth().logs(exits_filter).wait();
        if exit_events_filter_result.is_err() {
            return Err(String::from("Cant find exit event"))
        }
        let mut exit_events = exit_events_filter_result.unwrap();
        exit_events.sort_by(|l, r| {
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
            panic!("Logs can not have same indexes");
        });
        let mut this_batch: HashSet<U256> = HashSet::new();

        for event in exit_events {
            let account_id = U256::from(event.topics[2]);;
            let existing_record = this_batch.get(&account_id).map(|&v| v.clone());
            if let Some(record) = existing_record {
                return Err(String::from("Double exit should not be possible"))
            } else {
                this_batch.insert(account_id);
            }
            continue;
        }
        let mut all_exits = vec![];
        for k in this_batch.iter() {
            let exit: FullExitTransaction = FullExitTransaction{
                account_id: k.as_u32(),
            };
            all_exits.push(exit);
        }
        let block = FullExitTransactionsBlock {
            batch_number: batch_number.clone(),
            exits: all_exits,
        };
        Ok(block)
    }
}