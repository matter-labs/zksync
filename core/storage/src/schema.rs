table! {
    account_balance_updates (balance_update_id) {
        balance_update_id -> Int4,
        account_id -> Int8,
        block_number -> Int8,
        coin_id -> Int4,
        old_balance -> Numeric,
        new_balance -> Numeric,
        old_nonce -> Int8,
        new_nonce -> Int8,
        update_order_id -> Int4,
    }
}

table! {
    account_creates (account_id, block_number) {
        account_id -> Int8,
        is_create -> Bool,
        block_number -> Int8,
        address -> Bytea,
        nonce -> Int8,
        update_order_id -> Int4,
    }
}

table! {
    account_pubkey_updates (pubkey_update_id) {
        pubkey_update_id -> Int4,
        update_order_id -> Int4,
        account_id -> Int8,
        block_number -> Int8,
        old_pubkey_hash -> Bytea,
        new_pubkey_hash -> Bytea,
        old_nonce -> Int8,
        new_nonce -> Int8,
    }
}

table! {
    accounts (id) {
        id -> Int8,
        last_block -> Int8,
        nonce -> Int8,
        address -> Bytea,
        pubkey_hash -> Bytea,
    }
}

table! {
    active_provers (id) {
        id -> Int4,
        worker -> Text,
        created_at -> Timestamp,
        stopped_at -> Nullable<Timestamp>,
        block_size -> Int8,
    }
}

table! {
    balances (account_id, coin_id) {
        account_id -> Int8,
        coin_id -> Int4,
        balance -> Numeric,
    }
}

table! {
    blocks (number) {
        number -> Int8,
        root_hash -> Text,
        fee_account_id -> Int8,
        unprocessed_prior_op_before -> Int8,
        unprocessed_prior_op_after -> Int8,
        block_size -> Int8,
    }
}

table! {
    data_restore_last_watched_eth_block (id) {
        id -> Int4,
        block_number -> Text,
    }
}

table! {
    eth_nonce (id) {
        id -> Bool,
        nonce -> Int8,
    }
}

table! {
    eth_operations (id) {
        id -> Int8,
        op_id -> Int8,
        nonce -> Int8,
        deadline_block -> Int8,
        gas_price -> Numeric,
        tx_hash -> Bytea,
        confirmed -> Bool,
        raw_tx -> Bytea,
    }
}

table! {
    eth_stats (id) {
        id -> Bool,
        commit_ops -> Int8,
        verify_ops -> Int8,
        withdraw_ops -> Int8,
    }
}

table! {
    events_state (id) {
        id -> Int4,
        block_type -> Text,
        transaction_hash -> Bytea,
        block_num -> Int8,
    }
}

table! {
    executed_priority_operations (id) {
        id -> Int4,
        block_number -> Int8,
        block_index -> Int4,
        operation -> Jsonb,
        priority_op_serialid -> Int8,
        deadline_block -> Int8,
        eth_fee -> Numeric,
        eth_hash -> Bytea,
    }
}

table! {
    executed_transactions (id) {
        id -> Int4,
        block_number -> Int8,
        tx_hash -> Bytea,
        operation -> Nullable<Jsonb>,
        success -> Bool,
        fail_reason -> Nullable<Text>,
        block_index -> Nullable<Int4>,
    }
}

table! {
    mempool (hash) {
        hash -> Bytea,
        primary_account_address -> Bytea,
        nonce -> Int8,
        tx -> Jsonb,
        created_at -> Timestamp,
    }
}

table! {
    op_config (addr) {
        addr -> Text,
        next_nonce -> Nullable<Int8>,
    }
}

table! {
    operations (id) {
        id -> Int8,
        block_number -> Int8,
        action_type -> Text,
        created_at -> Timestamp,
        confirmed -> Bool,
    }
}

table! {
    proofs (block_number) {
        block_number -> Int8,
        proof -> Jsonb,
        created_at -> Timestamp,
    }
}

table! {
    prover_runs (id) {
        id -> Int4,
        block_number -> Int8,
        worker -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    rollup_ops (id) {
        id -> Int4,
        block_num -> Int8,
        operation -> Jsonb,
        fee_account -> Int8,
    }
}

table! {
    server_config (id) {
        id -> Bool,
        contract_addr -> Nullable<Text>,
        gov_contract_addr -> Nullable<Text>,
    }
}

table! {
    storage_state_update (id) {
        id -> Int4,
        storage_state -> Text,
    }
}

table! {
    tokens (id) {
        id -> Int4,
        address -> Text,
        symbol -> Text,
    }
}

joinable!(account_balance_updates -> tokens (coin_id));
joinable!(balances -> accounts (account_id));
joinable!(balances -> tokens (coin_id));
joinable!(eth_operations -> operations (op_id));
joinable!(executed_transactions -> mempool (tx_hash));

allow_tables_to_appear_in_same_query!(
    account_balance_updates,
    account_creates,
    account_pubkey_updates,
    accounts,
    active_provers,
    balances,
    blocks,
    data_restore_last_watched_eth_block,
    eth_nonce,
    eth_operations,
    eth_stats,
    events_state,
    executed_priority_operations,
    executed_transactions,
    mempool,
    op_config,
    operations,
    proofs,
    prover_runs,
    rollup_ops,
    server_config,
    storage_state_update,
    tokens,
);
