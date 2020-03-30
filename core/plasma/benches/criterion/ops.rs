// External uses
use criterion::{criterion_group, Criterion};
// Workspace uses
use models::node::AccountTree;
// Local uses
use plasma::state::PlasmaState;

/// Creates a `PlasmaState` object and fills it with accounts.
fn generate_state() -> PlasmaState {
    let depth = models::params::account_tree_depth() as u32;

    let mut accounts = AccountTree::new(depth);

    PlasmaState::empty()
}

pub fn bench_ops(_c: &mut Criterion) {}

criterion_group!(ops_benches, bench_ops);
