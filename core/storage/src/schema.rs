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
        created_at -> Timestamptz,
        stopped_at -> Nullable<Timestamptz>,
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
        root_hash -> Bytea,
        fee_account_id -> Int8,
        unprocessed_prior_op_before -> Int8,
        unprocessed_prior_op_after -> Int8,
        block_size -> Int8,
    }
}

table! {
    data_restore_events_state (id) {
        id -> Int4,
        block_type -> Text,
        transaction_hash -> Bytea,
        block_num -> Int8,
    }
}

table! {
    data_restore_last_watched_eth_block (id) {
        id -> Int4,
        block_number -> Text,
    }
}

table! {
    data_restore_rollup_ops (id) {
        id -> Int4,
        block_num -> Int8,
        operation -> Jsonb,
        fee_account -> Int8,
    }
}

table! {
    data_restore_storage_state_update (id) {
        id -> Int4,
        storage_state -> Text,
    }
}

table! {
    eth_operations (id) {
        id -> Int8,
        nonce -> Int8,
        confirmed -> Bool,
        raw_tx -> Bytea,
        op_type -> Text,
        final_hash -> Nullable<Bytea>,
        last_deadline_block -> Int8,
        last_used_gas_price -> Numeric,
    }
}

table! {
    eth_ops_binding (id) {
        id -> Int8,
        op_id -> Int8,
        eth_op_id -> Int8,
    }
}

table! {
    eth_parameters (id) {
        id -> Bool,
        nonce -> Int8,
        gas_price_limit -> Int8,
        commit_ops -> Int8,
        verify_ops -> Int8,
        withdraw_ops -> Int8,
    }
}

table! {
    eth_tx_hashes (id) {
        id -> Int8,
        eth_op_id -> Int8,
        tx_hash -> Bytea,
    }
}

table! {
    executed_priority_operations (id) {
        id -> Int4,
        block_number -> Int8,
        block_index -> Int4,
        operation -> Jsonb,
        from_account -> Bytea,
        to_account -> Bytea,
        priority_op_serialid -> Int8,
        deadline_block -> Int8,
        eth_hash -> Bytea,
        created_at -> Timestamptz,
    }
}

table! {
    executed_transactions (id) {
        id -> Int4,
        block_number -> Int8,
        block_index -> Nullable<Int4>,
        tx -> Jsonb,
        operation -> Jsonb,
        tx_hash -> Bytea,
        from_account -> Bytea,
        to_account -> Nullable<Bytea>,
        success -> Bool,
        fail_reason -> Nullable<Text>,
        primary_account_address -> Bytea,
        nonce -> Int8,
        created_at -> Timestamptz,
    }
}

table! {
    leader_election (id) {
        id -> Int4,
        name -> Text,
        created_at -> Timestamp,
        bail_at -> Nullable<Timestamp>,
    }
}

table! {
    operations (id) {
        id -> Int8,
        block_number -> Int8,
        action_type -> Text,
        created_at -> Timestamptz,
        confirmed -> Bool,
    }
}

table! {
    proofs (block_number) {
        block_number -> Int8,
        proof -> Jsonb,
        created_at -> Timestamptz,
    }
}

table! {
    prover_runs (id) {
        id -> Int4,
        block_number -> Int8,
        worker -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
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
    tokens (id) {
        id -> Int4,
        address -> Text,
        symbol -> Text,
        precision -> Int4,
    }
}

joinable!(account_balance_updates -> tokens (coin_id));
joinable!(balances -> accounts (account_id));
joinable!(balances -> tokens (coin_id));
joinable!(eth_ops_binding -> eth_operations (eth_op_id));
joinable!(eth_ops_binding -> operations (op_id));
joinable!(eth_tx_hashes -> eth_operations (eth_op_id));

allow_tables_to_appear_in_same_query!(
    account_balance_updates,
    account_creates,
    account_pubkey_updates,
    accounts,
    active_provers,
    balances,
    blocks,
    data_restore_events_state,
    data_restore_last_watched_eth_block,
    data_restore_rollup_ops,
    data_restore_storage_state_update,
    eth_operations,
    eth_ops_binding,
    eth_parameters,
    eth_tx_hashes,
    executed_priority_operations,
    executed_transactions,
    leader_election,
    operations,
    proofs,
    prover_runs,
    server_config,
    tokens,
);
