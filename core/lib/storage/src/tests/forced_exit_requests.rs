use std::str::FromStr;

use crate::misc::forced_exit_requests_schema::ForcedExitRequestsSchema;
use crate::tests::db_test;
use crate::QueryResult;
use crate::StorageProcessor;
use chrono::{DateTime, Timelike, Utc};
use num::{BigUint, FromPrimitive};
use zksync_types::misc::ForcedExitRequest;

#[db_test]
async fn store_forced_exit_request(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let now = Utc::now().with_nanosecond(0).unwrap();

    let request = ForcedExitRequest {
        id: 1,
        account_id: 12,
        tokens: vec![0],
        price_in_wei: BigUint::from_i32(121212).unwrap(),
        valid_until: DateTime::from(now),
    };

    ForcedExitRequestsSchema(&mut storage)
        .store_request(&request)
        .await?;

    let fe = ForcedExitRequestsSchema(&mut storage)
        .get_request_by_id(1)
        .await
        .expect("Failed to get forced exit by id");

    assert_eq!(request, fe);

    Ok(())
}

// #[db_test]
// async fn get_max_forced_exit_used_id(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
//     let now = Utc::now().with_nanosecond(0).unwrap();

//     let requests = [
//         ForcedExitRequest {
//             id: 1,
//             account_id: 1,
//             tokens: vec!(1),
//             price_in_wei: BigUint::from_i32(212).unwrap(),
//             valid_until: DateTime::from(now),
//         },
//         ForcedExitRequest {
//             id: 2,
//             account_id: 12,
//             tokens: vec!(0),
//             price_in_wei: BigUint::from_i32(1).unwrap(),
//             valid_until: DateTime::from(now),
//         },
//         ForcedExitRequest {
//             id: 7,
//             account_id: 3,
//             tokens: vec!(20),
//             price_in_wei: BigUint::from_str("1000000000000000").unwrap(),
//             valid_until: DateTime::from(now),
//         },
//     ];

//     for req in requests.iter() {
//         ForcedExitRequestsSchema(&mut storage)
//             .store_request(&req)
//             .await?;
//     }

//     let max_id = ForcedExitRequestsSchema(&mut storage)
//         .get_max_used_id()
//         .await
//         .expect("Failed to get forced exit by id");

//     assert_eq!(max_id, 7);

//     Ok(())
// }
