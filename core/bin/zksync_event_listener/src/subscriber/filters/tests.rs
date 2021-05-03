// Built-in uses
// External uses
// Workspace uses
use zksync_storage::event::EventType;
use zksync_types::{
    event::{account::*, block::*, test_data::*, transaction::*},
    AccountId, TokenId,
};
// Local uses
use super::{EventFilter, SubscriberFilters};

fn deserialize_valid(input: &str) -> SubscriberFilters {
    serde_json::from_str(input)
        .map_err(|err| eprintln!("Failed to deserialize valid input, error: {:?}", err))
        .unwrap()
}

/// Checks the [serde::Deserialize] implementation for filters.
/// The test is not supposed to be exhaustive since the trait is derived
/// for values of [EventFilter](super::EventFilter).
#[test]
fn test_filters_deserialize() {
    const INVALID: &[&str] = &[
        // Empty input.
        "",
        // A map is expected.
        "[]",
        // Deny unknown fields.
        r#"{
            "blocks": {}
        }"#,
        r#"{
            "block":
            {
                "block_status": "committed"
            }
        }"#,
        r#"{
            "accounts": {
                "tokens": [1, 2, 3]
            }
        }"#,
        // Check case-sensitivity.
        r#"{
            "transaction": {
                "types": [
                    "deposit"
                ]
            }
        }"#,
        r#"{
            "account": {
                "status": "Committed"
            },
            "block": {
                "status": "Reverted"
            }
        }"#,
        r#"{
            "transaction": {
                "status": "Rejected",
                "accounts": [1, 2, 3]
            }
        }"#,
    ];
    for (i, input) in INVALID.iter().enumerate() {
        let result = serde_json::from_str::<SubscriberFilters>(input);
        assert!(result.is_err(), "Input #{} is supposed to be invalid", i);
    }

    const VALID: &[&str] = &[
        "{}",
        r#"{
            "block": {}
        }"#,
        r#"{
            "block":
            {
                "status": "committed"
            }
        }"#,
        r#"{
            "transaction": {
                "types": [
                    "Deposit",
                    "ChangePubKey",
                    "Transfer",
                    "Withdraw",
                    "FullExit",
                    "ForcedExit"
                ],
                "status": "rejected"
            }
        }"#,
        r#"{
            "account": {
                "status": "committed",
                "accounts": [1, 2, 99, 99, 1000, 123987],
                "tokens": [0]
            },
            "block": {
                "status": "reverted"
            },
            "transaction": {}
        }"#,
    ];
    for (i, input) in VALID.iter().enumerate() {
        let result = serde_json::from_str::<SubscriberFilters>(input);
        assert!(
            result.is_ok(),
            "Failed to deserialize valid input #{}, error: {:?}",
            i,
            result
        );
    }
    // Check that common fields are deserialized correctly.
    const INPUT: &str = r#"
    {
        "block": {
            "status": "committed"
        },
        "transaction": {
            "status": "committed"
        },
        "account": {
            "status": "committed"
        }
    }"#;
    let filters: SubscriberFilters = deserialize_valid(INPUT);
    assert!(matches!(
        filters.0.get(&EventType::Account).unwrap(),
        EventFilter::Account(_)
    ));
    assert!(matches!(
        filters.0.get(&EventType::Block).unwrap(),
        EventFilter::Block(_)
    ));
    assert!(matches!(
        filters.0.get(&EventType::Transaction).unwrap(),
        EventFilter::Transaction(_)
    ));
}

/// Checks that [SubscriberFilters] accepts correct types of events.
/// Filtering by event properties is tested separately.
#[test]
fn test_subscriber_filters() {
    let account_event = get_account_event(
        AccountId(0),
        Some(TokenId(0)),
        AccountStateChangeStatus::Committed,
    );
    let block_event = get_block_event(BlockStatus::Committed);
    let tx_event = get_transaction_event(
        TransactionType::Transfer,
        AccountId(0),
        TokenId(0),
        TransactionStatus::Committed,
    );

    // Should accept all events.
    let filters = deserialize_valid("{}");
    assert!(filters.matches(&account_event));
    assert!(filters.matches(&block_event));
    assert!(filters.matches(&tx_event));

    // Only accept account event.
    let input = r#"{
        "account": {
            "accounts": [0],
            "tokens": [0],
            "status": "committed"
        }
    }"#;
    let filters = deserialize_valid(input);
    assert!(filters.matches(&account_event));
    // Block and tx events are not matched.
    assert!(!filters.matches(&block_event));
    assert!(!filters.matches(&tx_event));

    // Also accept block event.
    let input = r#"{
        "account": {},
        "block": {
            "status": "committed"
        }
    }"#;
    let filters = deserialize_valid(input);
    assert!(filters.matches(&account_event));
    assert!(filters.matches(&block_event));
    assert!(!filters.matches(&tx_event));

    // An alias for empty filters.
    let input = r#"{
        "account": {},
        "block": {},
        "transaction": {}
    }"#;
    let filters = deserialize_valid(input);
    assert!(filters.matches(&account_event));
    assert!(filters.matches(&block_event));
    assert!(filters.matches(&tx_event));
}
