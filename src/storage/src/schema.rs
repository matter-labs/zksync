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
    op_config,
    operations,
    proofs,
    prover_runs,
    server_config,
    transactions,
);
