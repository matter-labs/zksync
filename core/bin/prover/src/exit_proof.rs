//! Generate exit proof for exodus mode given account and token

use anyhow::format_err;
use log::info;
use num::BigUint;
use std::time::Instant;
use zksync_circuit::exit_circuit::create_exit_circuit_with_public_input;
use zksync_config::AvailableBlockSizesConfig;
use zksync_crypto::circuit::account::CircuitAccount;
use zksync_crypto::circuit::CircuitAccountTree;
use zksync_crypto::proof::EncodedAggregatedProof;
use zksync_prover_utils::aggregated_proofs::{gen_aggregate_proof, SingleProofData};
use zksync_prover_utils::{gen_verified_proof_for_exit_circuit, PlonkVerificationKey};
use zksync_types::{AccountId, AccountMap, Address, TokenId};

pub fn create_exit_proof(
    accounts: AccountMap,
    account_id: AccountId,
    owner: Address,
    token_id: TokenId,
) -> Result<(EncodedAggregatedProof, BigUint), anyhow::Error> {
    let timer = Instant::now();
    let mut circuit_account_tree =
        CircuitAccountTree::new(zksync_crypto::params::account_tree_depth());

    let mut target_account = None;
    for (id, account) in accounts {
        if id == account_id {
            target_account = Some(account.clone());
        }
        circuit_account_tree.insert(id, CircuitAccount::from(account));
    }

    let balance = target_account
        .map(|acc| acc.get_balance(token_id))
        .ok_or_else(|| {
            format_err!(
                "Fund account not found: id: {}, address: 0x{:x}",
                account_id,
                owner
            )
        })?;

    let zksync_exit_circuit =
        create_exit_circuit_with_public_input(&mut circuit_account_tree, account_id, token_id);

    let proof = gen_verified_proof_for_exit_circuit(zksync_exit_circuit)
        .map_err(|e| format_err!("Failed to generate proof: {}", e))?;

    let vk = PlonkVerificationKey::read_verification_key_for_exit_circuit()?;
    let aggreagated_proof = gen_aggregate_proof(
        vec![vk.0],
        vec![SingleProofData { proof, vk_idx: 0 }],
        &AvailableBlockSizesConfig::from_env().aggregated_proof_sizes_with_setup_pow(),
        false,
    )?;

    info!("Exit proof created: {} s", timer.elapsed().as_secs());
    Ok((aggreagated_proof.serialize_aggregated_proof(), balance))
}
