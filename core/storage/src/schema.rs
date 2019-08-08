table! {
    account_balance_updates (balance_update_id) {
        balance_update_id -> Int4,
        account_id -> Int4,
        block_number -> Int4,
        coin_id -> Int4,
        old_balance -> Numeric,
        new_balance -> Numeric,
        old_nonce -> Int8,
        new_nonce -> Int8,
    }
}

table! {
    account_creates (account_id, block_number) {
        account_id -> Int4,
        is_create -> Bool,
        block_number -> Int4,
        address -> Bytea,
        nonce -> Int8,
    }
}

table! {
    accounts (id) {
        id -> Int4,
        last_block -> Int4,
        nonce -> Int8,
        address -> Bytea,
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
    balances (account_id, coin_id) {
        account_id -> Int4,
        coin_id -> Int4,
        balance -> Numeric,
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
    }
}

table! {
    mempool (hash) {
        hash -> Bytea,
        primary_account_address -> Text,
        nonce -> Int8,
        tx -> Jsonb,
        created_at -> Timestamp,
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
    tokens (id) {
        id -> Int4,
        address -> Text,
    }
}

table! {
    transactions (id) {
        id -> Int4,
        tx_type -> Text,
        from_account -> Int4,
        to_account -> Nullable<Int4>,
        amount -> Int4,
        fee -> Int4,
        block_number -> Nullable<Int4>,
        state_root -> Nullable<Text>,
        created_at -> Timestamp,
        nonce -> Int4,
        token -> Int4,
    }
}

joinable!(account_balance_updates -> tokens (coin_id));
joinable!(balances -> accounts (account_id));
joinable!(balances -> tokens (coin_id));

allow_tables_to_appear_in_same_query!(
    account_balance_updates,
    account_creates,
    accounts,
    active_provers,
    balances,
    executed_transactions,
    mempool,
    op_config,
    operations,
    proofs,
    prover_runs,
    server_config,
    tokens,
    transactions,
);
