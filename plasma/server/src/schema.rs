table! {
    account_updates (account_id, block_number) {
        account_id -> Int4,
        block_number -> Int4,
        data -> Json,
    }
}

table! {
    accounts (id) {
        id -> Int4,
        last_block -> Int4,
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
        created_at -> Timestamp,
    }
}

allow_tables_to_appear_in_same_query!(
    account_updates,
    accounts,
    op_config,
    operations,
);
