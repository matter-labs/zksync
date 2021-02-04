use std::str::FromStr;

use crate::forced_exit_requests::ForcedExitRequestsSchema;
use crate::tests::db_test;
use crate::QueryResult;
use crate::StorageProcessor;
use chrono::{DateTime, Timelike, Utc};
use num::{BigUint, FromPrimitive};
use zksync_basic_types::Address;
use zksync_types::forced_exit_requests::{ForcedExitRequest, SaveForcedExitRequestQuery};

#[db_test]
async fn store_forced_exit_request(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let now = Utc::now().with_nanosecond(0).unwrap();

    let request = SaveForcedExitRequestQuery {
        target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
        tokens: vec![0],
        price_in_wei: BigUint::from_i32(121212).unwrap(),
        valid_until: DateTime::from(now),
    };

    let fe_request = ForcedExitRequestsSchema(&mut storage)
        .store_request(request)
        .await?;

    assert_eq!(fe_request.id, 1);

    let expected_response = ForcedExitRequest {
        id: 1,
        target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
        tokens: vec![0],
        price_in_wei: BigUint::from_i32(121212).unwrap(),
        valid_until: DateTime::from(now),
        fulfilled_at: None,
    };

    let response = ForcedExitRequestsSchema(&mut storage)
        .get_request_by_id(fe_request.id)
        .await
        .expect("Failed to get forced exit by id");

    assert_eq!(expected_response, response);
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
