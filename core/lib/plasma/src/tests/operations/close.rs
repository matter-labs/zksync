use crate::tests::PlasmaTestBuilder;
use models::node::{tx::TxSignature, Close};

#[test]
fn expected_fail() {
    let mut tb = PlasmaTestBuilder::new();

    let (_, account, _) = tb.add_account(false);
    let close = Close {
        account: account.address,
        nonce: account.nonce,
        signature: TxSignature::default(),
    };

    tb.test_tx_fail(close.into(), "Account closing is disabled");
}
