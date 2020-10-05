pub use simple::SimpleScenario;

use models::{tx::PackedEthSignature, FranklinTx};
use num::BigUint;

use crate::test_accounts::TestWallet;

mod simple;

pub trait Scenario {
    fn amount_to_deposit(&self) -> BigUint;

    fn initialize(&self, main_wallet: &TestWallet) -> anyhow::Result<()>;

    fn process(&self) -> anyhow::Result<Vec<(FranklinTx, Option<PackedEthSignature>)>>;

    fn finalize(&self, main_wallet: &TestWallet) -> anyhow::Result<()>;
}
