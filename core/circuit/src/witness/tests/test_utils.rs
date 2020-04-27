// External deps
use bigdecimal::BigDecimal;
use crypto_exports::franklin_crypto::{bellman::Circuit, circuit::test::TestConstraintSystem};
// Workspace deps
use models::{
    circuit::{account::CircuitAccount, CircuitAccountTree},
    node::{Account, AccountId, AccountMap, Address, Engine},
};
use plasma::state::PlasmaState;
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::circuit::FranklinCircuit;

// Public re-exports
pub use crate::witness::utils::WitnessBuilder;

pub const FEE_ACCOUNT_ID: u32 = 0;

/// Verifies that circuit has no unsatisfied constraints, and panics otherwise.
pub fn check_circuit(circuit: FranklinCircuit<Engine>) {
    let mut cs = TestConstraintSystem::<Engine>::new();
    circuit.synthesize(&mut cs).unwrap();

    println!("unconstrained: {}", cs.find_unconstrained());
    println!("number of constraints {}", cs.num_constraints());
    if let Some(err) = cs.which_is_unsatisfied() {
        panic!("ERROR satisfying in {}", err);
    }
}

/// Helper structure to generate `PlasmaState` and `CircuitAccountTree`.
#[derive(Debug)]
pub struct PlasmaStateGenerator;

impl PlasmaStateGenerator {
    fn create_state(accounts: AccountMap) -> (PlasmaState, CircuitAccountTree) {
        let plasma_state = PlasmaState::from_acc_map(accounts, 1);

        let mut circuit_account_tree =
            CircuitAccountTree::new(models::params::account_tree_depth());
        for (id, account) in plasma_state.get_accounts() {
            circuit_account_tree.insert(id, CircuitAccount::from(account))
        }

        (plasma_state, circuit_account_tree)
    }

    pub fn generate(accounts: &[WitnessTestAccount]) -> (PlasmaState, CircuitAccountTree) {
        let accounts: Vec<_> = accounts
            .iter()
            .map(|acc| (acc.id, acc.account.clone()))
            .collect();

        if accounts.iter().any(|(id, _)| *id == FEE_ACCOUNT_ID) {
            panic!("AccountId {} is an existing fee account", FEE_ACCOUNT_ID);
        }

        let validator_accounts = std::iter::once((
            FEE_ACCOUNT_ID,
            Account::default_with_address(&Address::default()),
        ))
        .chain(accounts)
        .collect();

        Self::create_state(validator_accounts)
    }

    pub fn from_single(account: &WitnessTestAccount) -> (PlasmaState, CircuitAccountTree) {
        if account.id == FEE_ACCOUNT_ID {
            panic!("AccountId {} is an existing fee account", FEE_ACCOUNT_ID);
        }

        let fee_account = (
            FEE_ACCOUNT_ID,
            Account::default_with_address(&Address::default()),
        );
        let validator_accounts = vec![fee_account, (account.id, account.account.clone())]
            .into_iter()
            .collect();

        Self::create_state(validator_accounts)
    }

    pub fn generate_empty() -> (PlasmaState, CircuitAccountTree) {
        Self::generate(&[])
    }
}

/// A helper structure for witness tests which contains both testkit
/// zkSync account and an actual zkSync account.
#[derive(Debug)]
pub struct WitnessTestAccount {
    pub zksync_account: ZksyncAccount,
    pub id: AccountId,
    pub account: Account,
}

impl WitnessTestAccount {
    pub fn new(id: AccountId, balance: u64) -> Self {
        let zksync_account = ZksyncAccount::rand();
        let account = {
            let mut account = Account::default_with_address(&zksync_account.address);
            account.add_balance(0, &BigDecimal::from(balance));
            account.pub_key_hash = zksync_account.pubkey_hash.clone();
            account
        };

        Self {
            zksync_account,
            id,
            account,
        }
    }

    pub fn new_empty(id: AccountId) -> Self {
        Self::new(id, 0)
    }
}
