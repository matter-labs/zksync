-- Drop all the indexes.
DROP INDEX operations_block_index;
DROP INDEX blocks_root_hash_index;
DROP INDEX tokens_symbol_index;
DROP INDEX eth_ops_binding_op_id_index;
DROP INDEX eth_tx_hashes_eth_op_id_index;
DROP INDEX mempool_txs_hash_index;

DROP INDEX accounts_block_index;
DROP INDEX accounts_address_index;
DROP INDEX account_balance_updates_block_index;
DROP INDEX account_creates_block_index;
DROP INDEX account_pubkey_updates_block_index;

DROP INDEX executed_transactions_block_number_index;
DROP INDEX executed_transactions_hash_index;
DROP INDEX executed_transactions_from_account_index;
DROP INDEX executed_transactions_to_account_index;

DROP INDEX executed_priority_operations_block_index;
DROP INDEX executed_priority_operations_serialid_index;
DROP INDEX executed_priority_operations_eth_hash_index;
DROP INDEX executed_priority_operations_from_account_index;
DROP INDEX executed_priority_operations_to_account_index;

-- Clear transactions/operations section
DROP TABLE IF EXISTS blocks;
DROP TABLE IF EXISTS pending_block;
DROP TABLE IF EXISTS operations;
DROP TABLE IF EXISTS executed_priority_operations;
DROP TABLE IF EXISTS rollup_ops;
DROP TABLE IF EXISTS mempool;
DROP TABLE IF EXISTS executed_transactions;

-- Clear accounts section
DROP TABLE IF EXISTS accounts;
DROP TABLE IF EXISTS account_balance_updates;
DROP TABLE IF EXISTS account_creates;
DROP TABLE IF EXISTS account_pubkey_updates;
DROP TABLE IF EXISTS balances;

-- Clear state section
DROP TABLE IF EXISTS events_state;
DROP TABLE IF EXISTS storage_state_update;

-- Clear prover section
DROP TABLE IF EXISTS proofs;
DROP TABLE IF EXISTS prover_runs;
DROP TABLE IF EXISTS active_provers;

-- Remove tokens section
DROP TABLE IF EXISTS tokens;

-- Remove server config table
DROP TABLE IF EXISTS server_config;

-- Clear ETH section
DROP TABLE IF EXISTS eth_operations;
DROP TABLE IF EXISTS eth_nonce;
DROP TABLE IF EXISTS eth_stats;
DROP TABLE IF EXISTS eth_ops_binding;
DROP TABLE IF EXISTS eth_tx_hashes;
DROP TABLE IF EXISTS data_restore_last_watched_eth_block;

-- Clear mempool section
DROP TABLE IF EXISTS mempool_txs;
