use std::str::FromStr;

use crate::forced_exit_requests::ForcedExitRequestsSchema;
use crate::tests::db_test;
use crate::QueryResult;
use crate::StorageProcessor;
use chrono::{Timelike, Utc};
use num::{BigUint, FromPrimitive};
use zksync_basic_types::Address;
use zksync_types::forced_exit_requests::{ForcedExitRequest, SaveForcedExitRequestQuery};

use std::ops::Add;

use zksync_types::TokenId;

#[db_test]
async fn get_oldest_unfulfilled_request(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let mut now = Utc::now().with_nanosecond(0).unwrap();

    // The requests have dummy created_at and valid_until values
    // They will reassigned in the future cycle
    let requests = vec![
        SaveForcedExitRequestQuery {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![TokenId(1)],
            price_in_wei: BigUint::from_i32(212).unwrap(),
            created_at: now,
            valid_until: now,
        },
        SaveForcedExitRequestQuery {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![TokenId(1)],
            price_in_wei: BigUint::from_i32(1).unwrap(),
            created_at: now,
            valid_until: now,
        },
        SaveForcedExitRequestQuery {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![TokenId(20)],
            price_in_wei: BigUint::from_str("1000000000000000").unwrap(),
            created_at: now,
            valid_until: now,
        },
    ];

    let mut stored_requests: Vec<ForcedExitRequest> = vec![];
    let interval = chrono::Duration::seconds(1);

    for req in requests.into_iter() {
        now = now.add(interval);
        let created_at = now;
        let valid_until = now.add(chrono::Duration::hours(32));

        stored_requests.push(
            ForcedExitRequestsSchema(&mut storage)
                .store_request(SaveForcedExitRequestQuery {
                    created_at,
                    valid_until,
                    ..req
                })
                .await
                .unwrap(),
        );
    }

    ForcedExitRequestsSchema(&mut storage)
        .set_fulfilled_at(stored_requests[0].id, Utc::now())
        .await?;

    let oldest_unfulfilled_request = ForcedExitRequestsSchema(&mut storage)
        .get_oldest_unfulfilled_request()
        .await?
        .unwrap();
    // The first request has been fulfilled. Thus, the second one should be the oldest
    assert_eq!(oldest_unfulfilled_request.id, stored_requests[1].id);

    // Now filling all the remaining requests
    ForcedExitRequestsSchema(&mut storage)
        .set_fulfilled_at(stored_requests[1].id, Utc::now())
        .await?;
    ForcedExitRequestsSchema(&mut storage)
        .set_fulfilled_at(stored_requests[2].id, Utc::now())
        .await?;

    let oldest_unfulfilled_request = ForcedExitRequestsSchema(&mut storage)
        .get_oldest_unfulfilled_request()
        .await?;
    // The first request has been fulfilled. Thus, the second one should be the oldest
    assert!(matches!(oldest_unfulfilled_request, None));

    Ok(())
}
