table! {
    accounts (id) {
        id -> Int4,
        last_block_number -> Nullable<Int4>,
        nonce -> Int4,
        balance -> Numeric,
        pub_x -> Nullable<Numeric>,
        pub_y -> Nullable<Numeric>,
    }
}

table! {
    blocks (block_number) {
        block_number -> Int4,
        block_data -> Json,
    }
}

allow_tables_to_appear_in_same_query!(
    accounts,
    blocks,
);
