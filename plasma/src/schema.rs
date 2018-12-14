table! {
    use diesel::sql_types::*;
    use crate::models::plasma_sql::sql_types::*;

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
    use diesel::sql_types::*;
    use crate::models::plasma_sql::sql_types::*;

    blocks (block_number) {
        block_number -> Int4,
        transactions -> Array<Tx>,
    }
}

allow_tables_to_appear_in_same_query!(
    accounts,
    blocks,
);
