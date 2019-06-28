table! {
    accounts (id) {
        id -> Int4,
        last_block -> Int4,
        data -> Json,
    }
}

table! {
    account_updates (account_id, block_number) {
        account_id -> Int4,
        block_number -> Int4,
        data -> Json,
    }
}

table! {
    active_provers (id) {
        id -> Int4,
        worker -> Text,
        created_at -> Timestamp,
        stopped_at -> Nullable<Timestamp>,
    }
}

table! {
    op_config (addr) {
        addr -> Text,
        next_nonce -> Nullable<Int4>,
    }
}

table! {
    operations (id) {
        id -> Int4,
        data -> Jsonb,
        addr -> Text,
        nonce -> Int4,
        block_number -> Int4,
        action_type -> Text,
        tx_hash -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

table! {
    proofs (block_number) {
        block_number -> Int4,
        proof -> Jsonb,
        created_at -> Timestamp,
    }
}

table! {
    prover_runs (id) {
        id -> Int4,
        block_number -> Int4,
        worker -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    server_config (id) {
        id -> Bool,
        contract_addr -> Nullable<Text>,
    }
}

table! {
    tree_restore_network (id) {
        id -> Int4,
        network_id -> Int2,
    }
}

table! {
    tree_restore_last_watched_eth_block (id) {
        id -> Int4,
        block_number -> Text,
    }
}

table! {
    block_events (id) {
        id -> Int4,
        block_type -> Text,
        transaction_hash -> Bytea,
        block_num -> Int8,
    }
}

table! {
    franklin_transactions (id) {
        id -> Int4,
        franklin_transaction_type -> Text,
        block_number -> Int8,
        eth_tx_hash -> Bytea,
        eth_tx_nonce -> Text,
        eth_tx_block_hash -> Nullable<Bytea>,
        eth_tx_block_number -> Nullable<Text>,
        eth_tx_transaction_index -> Nullable<Text>,
        eth_tx_from -> Bytea,
        eth_tx_to -> Nullable<Bytea>,
        eth_tx_value -> Text,
        eth_tx_gas_price -> Text,
        eth_tx_gas -> Text,
        eth_tx_input -> Bytea,
        commitment_data -> Bytea,
    }
}

table! {
    transactions (id) {
        id -> Int4,
        tx_type -> Text,
        from_account -> Int4,
        to_account -> Nullable<Int4>,
        nonce -> Nullable<Int4>,
        amount -> Int4,
        fee -> Int4,
        block_number -> Nullable<Int4>,
        state_root -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

allow_tables_to_appear_in_same_query!(
    accounts,
    account_updates,
    active_provers,
    op_config,
    operations,
    proofs,
    prover_runs,
    server_config,
    tree_restore_network,
    tree_restore_last_watched_eth_block,
    block_events,
    franklin_transactions,
    transactions,
);
