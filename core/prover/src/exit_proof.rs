//! Generate exit proof for exodus mode given account and token

use bigdecimal::BigDecimal;
use circuit::exit_circuit::create_exit_circuit_with_public_input;
use failure::format_err;
use log::info;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use models::node::{AccountMap, Address, TokenId};
use models::prover_utils::{gen_verified_proof_for_exit_circuit, EncodedProofPlonk};
use std::time::Instant;

pub fn create_exit_proof(
    accounts: AccountMap,
    owner: Address,
    token_id: TokenId,
) -> Result<(EncodedProofPlonk, BigDecimal), failure::Error> {
    let timer = Instant::now();
    let mut circuit_account_tree = CircuitAccountTree::new(models::params::account_tree_depth());

    let mut target_account = None;
    for (id, account) in accounts {
        if account.address == owner {
            target_account = Some((id, account.clone()));
        }
        circuit_account_tree.insert(id, CircuitAccount::from(account));
    }

    let (account_id, balance) = target_account
        .map(|(id, acc)| (id, acc.get_balance(token_id)))
        .ok_or_else(|| format_err!("Fund account not found: 0x{:x}", owner))?;

    let zksync_exit_circuit =
        create_exit_circuit_with_public_input(&mut circuit_account_tree, account_id, token_id);

    let proof = gen_verified_proof_for_exit_circuit(zksync_exit_circuit)
        .map_err(|e| format_err!("Failed to generate proof: {}", e))?;

    info!("Exit proof created: {} s", timer.elapsed().as_secs());
    Ok((proof, balance))
}
