//! Generate exit proof for exodus mode given account and token

use bigdecimal::BigDecimal;
use circuit::exit_circuit::create_exit_circuit_with_public_input;
use crypto_exports::bellman::groth16::Parameters;
use failure::{ensure, format_err};
use log::info;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use models::node::{AccountMap, Address, Engine, TokenId};
use models::prover_utils::{
    create_random_full_baby_proof, encode_proof, get_exodus_proof_key_and_vk_path,
    read_circuit_proving_parameters, verify_full_baby_proof,
};
use models::EncodedProof;
use std::time::Instant;

fn read_parameters() -> Result<Parameters<Engine>, failure::Error> {
    let path = get_exodus_proof_key_and_vk_path().0;
    Ok(read_circuit_proving_parameters(&path)?)
}

pub fn create_exit_proof(
    accounts: AccountMap,
    owner: Address,
    token_id: TokenId,
) -> Result<(EncodedProof, BigDecimal), failure::Error> {
    let timer = Instant::now();
    let mut circuit_account_tree =
        CircuitAccountTree::new(models::params::account_tree_depth() as u32);

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

    let parameters = read_parameters()?;
    let (zksync_exit_circuit, public_input) =
        create_exit_circuit_with_public_input(&mut circuit_account_tree, account_id, token_id);
    let proof = create_random_full_baby_proof(zksync_exit_circuit, public_input, &parameters)
        .map_err(|e| format_err!("Failed to generate proof: {}", e))?;

    ensure!(
        verify_full_baby_proof(&proof, &parameters)
            .map_err(|e| format_err!("Failed to verify proof: {}", e))?,
        "Proof is invalid"
    );

    let proof_for_ethereum = encode_proof(&proof.proof);

    info!("Exit proof created: {} s", timer.elapsed().as_secs());
    Ok((proof_for_ethereum, balance))
}
