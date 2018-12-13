table! {
    account (id) {
        id -> Int4,
        last_block_number -> Nullable<Int4>,
        nonce -> Nullable<Int8>,
        amount -> Nullable<Numeric>,
        pub_x -> Nullable<Numeric>,
    }
}

table! {
    block (block_number) {
        block_number -> Int4,
        tx_type -> Op_type,
        created_at -> Timestamp,
        root_hash -> Nullable<Numeric>,
        transactions -> Array<Tx>,
    }
}

allow_tables_to_appear_in_same_query!(
    account,
    block,
);
