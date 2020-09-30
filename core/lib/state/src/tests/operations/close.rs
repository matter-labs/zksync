use crate::tests::{AccountState::*, PlasmaTestBuilder};
use zksync_types::tx::{Close, TxSignature};

/// Checks that Close operations fails
/// because it is disabled
#[test]
fn expected_fail() {
    let mut tb = PlasmaTestBuilder::new();

    let (_, account, _) = tb.add_account(Locked);
    let close = Close {
        account: account.address,
        nonce: account.nonce,
        signature: TxSignature::default(),
    };

    tb.test_tx_fail(close.into(), "Account closing is disabled");
}
