use sqlx::types::BigDecimal;
use zksync_types::TokenId;

use crate::tests::db_test;
use crate::{misc::records::Subsidy, misc::MiscSchema};
use crate::{QueryResult, StorageProcessor};

fn get_subsidy(name: String, value: u64) -> Subsidy {
    // The only fields that matter are `subsidy_type` and `value`
    Subsidy {
        tx_hash: Default::default(),
        usd_amount_scaled: value,
        full_cost_usd_scaled: 2 * value,
        token_id: TokenId(0),
        token_amount: BigDecimal::from(100),
        full_cost_token: BigDecimal::from(200),
        subsidy_type: name,
    }
}

/// Checks that storing and loading the last watched block number
/// works as expected.
#[db_test]
async fn stored_subsidy(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let subsidy_name = "subsidy".to_string();
    let another_subsidy_name = "another_subsidy".to_string();

    let subsidy_1 = get_subsidy(subsidy_name.clone(), 10);
    let subsidy_2 = get_subsidy(another_subsidy_name.clone(), 45);
    let subsidy_3 = get_subsidy(subsidy_name.clone(), 15);

    let get_total_subsidy = MiscSchema(&mut storage)
        .get_total_used_subsidy_for_type(&subsidy_name)
        .await?;
    assert_eq!(get_total_subsidy, BigDecimal::from(0));

    MiscSchema(&mut storage).store_subsidy(subsidy_1).await?;
    let get_total_subsidy = MiscSchema(&mut storage)
        .get_total_used_subsidy_for_type(&subsidy_name)
        .await?;
    assert_eq!(get_total_subsidy, BigDecimal::from(10));

    MiscSchema(&mut storage).store_subsidy(subsidy_2).await?;
    let get_total_subsidy = MiscSchema(&mut storage)
        .get_total_used_subsidy_for_type(&subsidy_name)
        .await?;
    assert_eq!(get_total_subsidy, BigDecimal::from(10));

    MiscSchema(&mut storage).store_subsidy(subsidy_3).await?;
    let get_total_subsidy = MiscSchema(&mut storage)
        .get_total_used_subsidy_for_type(&subsidy_name)
        .await?;
    assert_eq!(get_total_subsidy, BigDecimal::from(25));

    Ok(())
}
