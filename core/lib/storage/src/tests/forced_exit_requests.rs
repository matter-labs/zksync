use std::{
    ops::{Mul, Sub},
    str::FromStr,
};

use crate::forced_exit_requests::ForcedExitRequestsSchema;
use crate::tests::db_test;
use crate::QueryResult;
use crate::StorageProcessor;
use chrono::{Duration, Timelike, Utc};
use num::{BigUint, FromPrimitive};
use zksync_basic_types::Address;
use zksync_types::{
    forced_exit_requests::{ForcedExitRequest, SaveForcedExitRequestQuery},
    tx::TxHash,
};

use std::ops::Add;

use zksync_types::TokenId;

// Accepts an array of requests and stores them in the db
pub async fn store_requests(
    storage: &mut StorageProcessor<'_>,
    requests: Vec<SaveForcedExitRequestQuery>,
) -> Vec<ForcedExitRequest> {
    let mut stored_requests: Vec<ForcedExitRequest> = vec![];
    for req in requests.into_iter() {
        stored_requests.push(
            ForcedExitRequestsSchema(storage)
                .store_request(req)
                .await
                .unwrap(),
        );
    }
    stored_requests
}

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

// Checks that during deletion of the old transactions
// are deleted and no more
#[db_test]
async fn delete_old_requests(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let now = Utc::now().with_nanosecond(0).unwrap();

    let deleting_threshold = Duration::days(3);
    let day = Duration::days(1);
    let minute = Duration::minutes(1);

    // So here we imagine that the requests are valid for 2 days
    // and we delete the old requests after at least 3 days have expired
    let requests = vec![
        SaveForcedExitRequestQuery {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![TokenId(1)],
            price_in_wei: BigUint::from_i32(212).unwrap(),
            created_at: now.sub(day.mul(8)),
            // Invalid for 6 days => should be deleted
            valid_until: now.sub(day.mul(6)),
        },
        SaveForcedExitRequestQuery {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![TokenId(1)],
            price_in_wei: BigUint::from_i32(1).unwrap(),
            created_at: now.sub(day.mul(5)).sub(minute),
            // Invalid for 3 days and 1 minutes => should be deleted
            valid_until: now.sub(day.mul(3)).sub(minute),
        },
        SaveForcedExitRequestQuery {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![TokenId(20)],
            price_in_wei: BigUint::from_str("1000000000000000").unwrap(),
            created_at: now.sub(day.mul(5)).add(minute.mul(5)),
            // Invalid for 3 days minus 5 minutes => should not be deleted
            valid_until: now.sub(day.mul(3)).add(minute.mul(5)),
        },
        SaveForcedExitRequestQuery {
            target: Address::from_str("c0f97CC918C9d6fA4E9fc6be61a6a06589D199b2").unwrap(),
            tokens: vec![TokenId(20)],
            price_in_wei: BigUint::from_str("1000000000000000").unwrap(),
            created_at: now.sub(day.mul(5)).add(minute.mul(5)),
            // Is valid => should not be deleted
            valid_until: now.sub(day.mul(3)).add(minute.mul(5)),
        },
    ];

    let stored_requests = store_requests(&mut storage, requests).await;

    // This a hash of a random transaction
    let transaction_hash = TxHash::from_str(
        "sync-tx:796018689b3e323894f44fb0093856ec3832908c626dea357a9bd1b25f9d11bf",
    )
    .unwrap();

    // Setting fullfilled_by for the oldest request
    // so that it should not be deleted
    ForcedExitRequestsSchema(&mut storage)
        .set_fulfilled_by(stored_requests[0].id, Some(vec![transaction_hash]))
        .await?;

    ForcedExitRequestsSchema(&mut storage)
        .delete_old_unfulfilled_requests(deleting_threshold)
        .await?;

    // true means should not have been deleted
    // false means should have been deleted
    // Note that we have set the fulfilled_by for the first tx, that's why it should
    // not have been deleted
    let should_remain = vec![true, false, true, true];

    for (i, request) in stored_requests.into_iter().enumerate() {
        let stored = ForcedExitRequestsSchema(&mut storage)
            .get_request_by_id(request.id)
            .await?;

        let processed_correctly = if should_remain[i] {
            stored.is_some()
        } else {
            stored.is_none()
        };

        assert!(processed_correctly, "Deletion was not processed correctly");
    }

    Ok(())
}
