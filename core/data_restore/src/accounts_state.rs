use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;

use ff::{Field, PrimeField, PrimeFieldRepr};
use sapling_crypto::jubjub::{edwards, Unknown};
use web3::futures::Future;
use web3::types::{BlockNumber, Filter, FilterBuilder, H256, U256};

use bigdecimal::{BigDecimal, Num, Zero};

use models::plasma::params;
use models::plasma::tx::{DepositTx, ExitTx, TransferTx, TxSignature};
use models::plasma::{Account, AccountId};
use models::plasma::{Engine, Fr};
use plasma::state::PlasmaState;
use web3::types::Log;

use crate::franklin_transaction::{FranklinTransaction, FranklinTransactionType};
use crate::helpers::*;

pub struct FranklinAccountsStates {
    pub config: DataRestoreConfig,
    pub plasma_state: PlasmaState,
}

impl FranklinAccountsStates {
    pub fn new(config: DataRestoreConfig) -> Self {
        Self {
            config,
            plasma_state: PlasmaState::empty(),
        }
    }

    pub fn update_accounts_states_from_transaction(
        &mut self,
        transaction: &FranklinTransaction,
    ) -> Result<(), DataRestoreError> {
        let tx_type = transaction.franklin_transaction_type;
        match tx_type {
            FranklinTransactionType::Deposit => {
                Ok(self.update_accounts_states_from_deposit_transaction(transaction)?)
            }
            FranklinTransactionType::FullExit => {
                Ok(self.update_accounts_states_from_full_exit_transaction(transaction)?)
            }
            FranklinTransactionType::Transfer => {
                Ok(self.update_accounts_states_from_transfer_transaction(transaction)?)
            }
            _ => Err(DataRestoreError::WrongType),
        }
    }

    pub fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.plasma_state.get_accounts()
    }

    pub fn root_hash(&self) -> Fr {
        self.plasma_state.root_hash()
    }

    pub fn get_account(&self, account_id: AccountId) -> Option<Account> {
        self.plasma_state.get_account(account_id)
    }

    fn update_accounts_states_from_transfer_transaction(
        &mut self,
        transaction: &FranklinTransaction,
    ) -> Result<(), DataRestoreError> {
        // debug!("tx: {:?}", transaction.ethereum_transaction.hash);
        let transfer_txs_block = self
            .get_all_transactions_from_transfer_block(transaction)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        for tx in transfer_txs_block {
            if let Some(mut from) = self.plasma_state.balance_tree.items.get(&tx.from).cloned() {
                let mut transacted_amount = BigDecimal::zero();
                // debug!("amount tx: {:?}", &tx.amount);
                transacted_amount += &tx.amount;
                transacted_amount += &tx.fee;

                if from.balance < transacted_amount {
                    return Err(DataRestoreError::WrongAmount);
                }

                let mut to = Account::default();
                if let Some(existing_to) = self.plasma_state.balance_tree.items.get(&tx.to) {
                    to = existing_to.clone();
                }

                from.balance -= transacted_amount;

                from.nonce += 1;
                if tx.to != 0 {
                    to.balance += &tx.amount;
                }

                self.plasma_state.balance_tree.insert(tx.from, from);
                self.plasma_state.balance_tree.insert(tx.to, to);
            } else {
                return Err(DataRestoreError::NonexistentAccount);
            }
        }
        Ok(())
    }

    fn update_accounts_states_from_deposit_transaction(
        &mut self,
        transaction: &FranklinTransaction,
    ) -> Result<(), DataRestoreError> {
        let batch_number = self.get_batch_number(transaction);
        // let block_number = self.get_block_number_from_deposit(transaction);
        let deposit_txs_block = self
            .get_all_transactions_from_deposit_batch(batch_number)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        for tx in deposit_txs_block {
            let mut account = self
                .plasma_state
                .balance_tree
                .items
                .remove(&tx.account)
                .unwrap_or_else(|| {
                    let mut new_account = Account::default();
                    new_account.public_key_x = tx.pub_x;
                    new_account.public_key_y = tx.pub_y;
                    new_account.balance = BigDecimal::zero();
                    new_account
                });
            account.balance += tx.amount;
            self.plasma_state.balance_tree.insert(tx.account, account);
        }
        Ok(())
    }

    fn update_accounts_states_from_full_exit_transaction(
        &mut self,
        transaction: &FranklinTransaction,
    ) -> Result<(), DataRestoreError> {
        let batch_number = self.get_batch_number(transaction);
        // let block_number = self.get_block_number_from_full_exit(transaction);
        let exit_txs_block = self
            .get_all_transactions_from_full_exit_batch(batch_number)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        for tx in exit_txs_block {
            let _acc = self
                .plasma_state
                .balance_tree
                .items
                .get(&tx.account)
                .cloned();
            if _acc.is_none() {
                return Err(DataRestoreError::NonexistentAccount);
            }
            self.plasma_state.balance_tree.delete(tx.account);
        }
        Ok(())
    }

    fn get_batch_number(&self, transaction: &FranklinTransaction) -> U256 {
        U256::from(&transaction.commitment_data[0..32])
    }

    // fn get_block_number_from_deposit(&self, transaction: &FranklinTransaction) -> U256 {
    //     let block_vec = transaction.commitment_data[64..96].to_vec();
    //     let block_slice = block_vec.as_slice();
    //     U256::from(block_slice)
    // }

    // fn get_block_number_from_full_exit(&self, transaction: &FranklinTransaction) -> U256 {
    //     let block_vec = transaction.commitment_data[64..96].to_vec();
    //     let block_slice = block_vec.as_slice();
    //     U256::from(block_slice)
    // }

    pub fn get_all_transactions_from_transfer_block(
        &self,
        transaction: &FranklinTransaction,
    ) -> Result<Vec<TransferTx>, DataRestoreError> {
        let mut tx_data_vec = transaction.commitment_data.clone();
        // debug!("tx_data_vec: {:?}", tx_data_vec);
        // let block_number = &transaction.commitment_data.clone()[0..32];
        // debug!("block_number: {:?}", block_number);
        let tx_data_len = tx_data_vec.len();
        // debug!("tx_data_len: {:?}", tx_data_len);
        tx_data_vec.reverse();
        tx_data_vec.truncate(tx_data_len - 160);
        tx_data_vec.reverse();
        // debug!("tx_data_vec final: {:?}", tx_data_vec);
        // tx_data_len = tx_data_vec.len();
        // debug!("tx_data_len final: {:?}", tx_data_len);
        let txs = tx_data_vec.chunks(9);

        let mut transfers: Vec<TransferTx> = vec![];
        for (i, tx) in txs.enumerate() {
            // if tx != [0, 0, 2, 0, 0, 0, 0, 0, 0] {
            //     let from = U256::from(&tx[0..3]);
            //     let to = U256::from(&tx[3..6]);
            //     let amount = U256::from(&tx[6..8]);
            //     let fee = U256::from(tx[8]);
            //     let transfer_tx = TransferTx {
            //         from: from.as_u32(),
            //         to: to.as_u32(),
            //         amount: BigDecimal::from_str_radix(&format!("{}", amount), 10).unwrap(),
            //         fee: BigDecimal::from_str_radix(&format!("{}", fee), 10).unwrap(),
            //         nonce: 0,
            //         good_until_block: 0,
            //         signature: TxSignature::default(),
            //         cached_pub_key: None,
            //     };
            //     transfers.push(transfer_tx);
            // }
            let from = U256::from(&tx[0..3]).as_u32();
            let to = U256::from(&tx[3..6]).as_u32();
            let amount = amount_bytes_slice_to_big_decimal(&tx[6..8]);
            let fee = fee_bytes_slice_to_big_decimal(tx[8]);
            let transfer_tx = TransferTx {
                from,
                to,
                amount: amount.clone(), //BigDecimal::from_str_radix("0", 10).unwrap(),
                fee,                    //BigDecimal::from_str_radix("0", 10).unwrap(),
                nonce: i.try_into().expect("Cant make nonce in get_all_transactions_from_transfer_block"),
                good_until_block: 0,
                signature: TxSignature::default(),
                cached_pub_key: None,
            };
            debug!(
                "Transaction from account {:?} to account {:?}, amount = {:?}",
                from, to, amount
            );
            transfers.push(transfer_tx);
        }

        Ok(transfers)
    }

    fn load_sorted_events(
        &self,
        action_filter: Filter,
        cancel_filter: Filter,
    ) -> Result<Vec<Log>, DataRestoreError> {
        let (_eloop, transport) = web3::transports::Http::new(self.config.web3_endpoint.as_str())
            .map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let action_events = web3
            .eth()
            .logs(action_filter)
            .wait()
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let cancel_events = web3
            .eth()
            .logs(cancel_filter)
            .wait()
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;

        // now we have to merge and apply
        let mut all_events = vec![];
        all_events.extend(action_events.into_iter());
        all_events.extend(cancel_events.into_iter());

        all_events = all_events
            .into_iter()
            .filter(|el| !el.is_removed())
            .collect();

        // sort by index
        let mut error_flag = false;
        all_events.sort_by(|l, r| {
            let l_block = l.block_number.expect("Cant sort blocks in load_sorted_events");
            let r_block = r.block_number.expect("Cant sort blocks in load_sorted_events");

            let l_index = l.log_index.expect("Cant sort logs in load_sorted_events");
            let r_index = r.log_index.expect("Cant sort logs in load_sorted_events");

            let ordering = l_block.cmp(&r_block).then(l_index.cmp(&r_index));
            if ordering == Ordering::Equal {
                error_flag = true;
            }
            ordering
        });
        if error_flag {
            return Err(DataRestoreError::Unknown(
                "Logs can not have same indexes".to_string(),
            ));
        }
        Ok(all_events)
    }

    pub fn get_all_transactions_from_deposit_batch(
        &self,
        batch_number: U256,
    ) -> Result<Vec<DepositTx>, DataRestoreError> {
        let deposit_event = self
            .config
            .franklin_contract
            .event("LogDepositRequest")
            .expect("Cant create deposit event in get_all_transactions_from_deposit_batch")
            .clone();
        let deposit_event_topic = deposit_event.signature();

        let deposit_canceled_event = self
            .config
            .franklin_contract
            .event("LogCancelDepositRequest")
            .expect("Cant create deposit canceled event in get_all_transactions_from_deposit_batch")
            .clone();
        let deposit_canceled_topic = deposit_canceled_event.signature();

        let deposits_filter = FilterBuilder::default()
            .address(vec![self.config.franklin_contract_address])
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .topics(
                Some(vec![deposit_event_topic]),
                Some(vec![H256::from(batch_number)]),
                None,
                None,
            )
            .build();
        let cancels_filter = FilterBuilder::default()
            .address(vec![self.config.franklin_contract_address])
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .topics(
                Some(vec![deposit_canceled_topic]),
                Some(vec![H256::from(batch_number)]),
                None,
                None,
            )
            .build();

        let all_events = self.load_sorted_events(deposits_filter, cancels_filter)?;

        let mut this_batch: HashMap<U256, (U256, U256)> = HashMap::new();

        for event in all_events {
            let topic = event.topics[0];
            match () {
                () if topic == deposit_event_topic => {
                    let data_bytes: Vec<u8> = event.data.0;
                    let account_id = U256::from(event.topics[2]);
                    let public_key = U256::from(event.topics[3]);
                    let deposit_amount = U256::from_big_endian(&data_bytes);
                    let _existing_record = this_batch.get(&account_id).cloned();
                    if let Some(record) = _existing_record {
                        let mut existing_balance = record.0;
                        existing_balance += deposit_amount;
                        this_batch.insert(account_id, (existing_balance, record.1));
                    } else {
                        this_batch.insert(account_id, (deposit_amount, public_key));
                    }
                    continue;
                }
                () if topic == deposit_canceled_topic => {
                    let account_id = U256::from(event.topics[2]);
                    let _existing_record = this_batch
                        .get(&account_id)
                        .cloned()
                        .ok_or("existing_record not found for deposits")?;
                    this_batch.remove(&account_id);
                    continue;
                }
                _ => return Err(DataRestoreError::Unknown("unexpected topic".to_string())),
            }
        }

        let mut all_deposits = vec![];
        for (k, v) in this_batch.iter() {
            debug!(
                "Into account {:?} with public key {:x}, deposit amount = {:?}",
                k, v.1, v.0
            );
            let mut public_key_bytes = vec![0u8; 32];
            v.1.to_big_endian(&mut public_key_bytes);
            let x_sign = public_key_bytes[0] & 0x80 > 0;
            public_key_bytes[0] &= 0x7f;
            let mut fe_repr = Fr::zero().into_repr();
            fe_repr
                .read_be(public_key_bytes.as_slice())
                .expect("read public key point");
            let y = Fr::from_repr(fe_repr);
            if y.is_err() {
                return Err(DataRestoreError::WrongPubKey);
            }
            let public_key_point = edwards::Point::<Engine, Unknown>::get_for_y(
                y.expect("Cant create public_key_point in get_all_transactions_from_deposit_batch"),
                x_sign,
                &params::JUBJUB_PARAMS,
            );
            if public_key_point.is_none() {
                return Err(DataRestoreError::WrongPubKey);
            }

            let (pub_x, pub_y) = public_key_point.expect("Cant create x and y in get_all_transactions_from_deposit_batch").into_xy();

            let tx: DepositTx = DepositTx {
                account: k.as_u32(),
                amount: BigDecimal::from_str_radix(&format!("{}", v.0), 10).expect("Cant create amount in get_all_transactions_from_deposit_batch"),
                pub_x,
                pub_y,
            };
            all_deposits.push(tx);
        }
        Ok(all_deposits)
    }

    pub fn get_all_transactions_from_full_exit_batch(
        &self,
        batch_number: U256,
    ) -> Result<Vec<ExitTx>, DataRestoreError> {
        let exit_event = self
            .config
            .franklin_contract
            .event("LogExitRequest")
            .expect("Cant create exit event in get_all_transactions_from_full_exit_batch")
            .clone();
        let exit_event_topic = exit_event.signature();

        let exit_canceled_event = self
            .config
            .franklin_contract
            .event("LogCancelExitRequest")
            .expect("Cant create exit canceled event in get_all_transactions_from_full_exit_batch")
            .clone();
        let exit_canceled_topic = exit_canceled_event.signature();

        let exits_filter = FilterBuilder::default()
            .address(vec![self.config.franklin_contract_address])
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .topics(
                Some(vec![exit_event_topic]),
                Some(vec![H256::from(batch_number)]),
                None,
                None,
            )
            .build();

        let cancels_filter = FilterBuilder::default()
            .address(vec![self.config.franklin_contract_address])
            .from_block(BlockNumber::Earliest)
            .to_block(BlockNumber::Latest)
            .topics(
                Some(vec![exit_canceled_topic]),
                Some(vec![H256::from(batch_number)]),
                None,
                None,
            )
            .build();

        let all_events = self.load_sorted_events(exits_filter, cancels_filter)?;

        let mut this_batch: HashSet<U256> = HashSet::new();

        for event in all_events {
            let topic = event.topics[0];
            match () {
                () if topic == exit_event_topic => {
                    let account_id = U256::from(event.topics[2]);
                    let existing_record = this_batch.get(&account_id).cloned();
                    if existing_record.is_some() {
                        return Err(DataRestoreError::DoubleExit);
                    } else {
                        this_batch.insert(account_id);
                    }
                    continue;
                }
                () if topic == exit_canceled_topic => {
                    let account_id = U256::from(event.topics[2]);
                    this_batch
                        .get(&account_id)
                        .cloned()
                        .ok_or_else(|| "existing_record fetch failed".to_owned())?;
                    this_batch.remove(&account_id);
                    continue;
                }
                _ => return Err(DataRestoreError::Unknown("unexpected topic".to_string())),
            }
        }

        let mut all_exits = vec![];
        for k in this_batch.iter() {
            debug!("Exit from account {:?}", k);

            let tx: ExitTx = ExitTx {
                account: k.as_u32(),
                amount: BigDecimal::zero(),
            };
            all_exits.push(tx);
        }

        Ok(all_exits)
    }
}
