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


