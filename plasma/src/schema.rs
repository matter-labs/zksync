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
        data -> Json,
    }
}

table! {
    operations (id) {
        id -> Int4,
        block_number -> Int4,
        data -> Jsonb,
        addr -> Text,
        nonce -> Int4,
        created_at -> Timestamp,
    }
}

allow_tables_to_appear_in_same_query!(
    account_updates,
    accounts,
    operations,
);
