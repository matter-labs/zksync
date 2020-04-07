-- Clear transactions/operations section
DROP TABLE IF EXISTS blocks;
DROP TABLE IF EXISTS operations;
DROP TABLE IF EXISTS executed_transactions;
DROP TABLE IF EXISTS executed_priority_operations;
DROP TABLE IF EXISTS rollup_ops;
DROP TABLE IF EXISTS mempool;

-- Clear accounts section
DROP TABLE IF EXISTS accounts;
DROP TABLE IF EXISTS account_balance_updates;
DROP TABLE IF EXISTS account_creates;
DROP TABLE IF EXISTS account_pubkey_updates;
DROP TABLE IF EXISTS balances;

-- Clear state section
DROP TABLE IF EXISTS storage_state_update;
DROP TABLE IF EXISTS events_state;

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
