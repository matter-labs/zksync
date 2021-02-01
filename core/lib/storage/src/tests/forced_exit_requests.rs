use crate::misc::forced_exit_requests_schema::ForcedExitRequestsSchema;
use crate::tests::db_test;
use crate::QueryResult;
use crate::{chain::operations::OperationsSchema, ethereum::EthereumSchema, StorageProcessor};
use chrono::{DateTime, Utc};
use num::{BigInt, BigUint, FromPrimitive};
use zksync_types::misc::ForcedExitRequest;

#[db_test]
async fn store_forced_exit_request(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let now = Utc::now();

    let request = ForcedExitRequest {
        id: 1,
        account_id: 12,
        token_id: 0,
        price_in_wei: BigUint::from_i32(121212).unwrap(),
        valid_until: DateTime::from(now),
    };

    ForcedExitRequestsSchema(&mut storage).store_request(&request);

    let fe = ForcedExitRequestsSchema(&mut storage)
        .get_request_by_id(1)
        .await
        .expect("Failed to get forced exit by id");

    assert_eq!(request, fe);

    Ok(())
}
