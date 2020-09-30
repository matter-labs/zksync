use super::{SETUP_MAX_POW2, SETUP_MIN_POW2};
use anyhow::format_err;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use zksync_crypto::bellman::kate_commitment::{Crs, CrsForLagrangeForm, CrsForMonomialForm};
use zksync_crypto::params::{account_tree_depth, balance_tree_depth};
use zksync_crypto::Engine;

pub fn get_keys_root_dir() -> PathBuf {
    let mut out_dir = PathBuf::new();
    out_dir.push(&std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| "/".to_owned()));
    out_dir.push(&std::env::var("KEY_DIR").expect("KEY_DIR not set"));
    out_dir.push(&format!(
        "account-{}_balance-{}",
        account_tree_depth(),
        balance_tree_depth(),
    ));
    out_dir
}

fn base_universal_setup_dir() -> Result<PathBuf, anyhow::Error> {
    let mut dir = PathBuf::new();
    // root is used by default for provers
    dir.push(&std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| "/".to_owned()));
    dir.push("keys");
    dir.push("setup");
    anyhow::ensure!(dir.exists(), "Universal setup dir does not exits");
    Ok(dir)
}

fn get_universal_setup_file_buff_reader(
    setup_file_name: &str,
) -> Result<BufReader<File>, anyhow::Error> {
    let setup_file = {
        let mut path = base_universal_setup_dir()?;
        path.push(&setup_file_name);
        File::open(path).map_err(|e| {
            format_err!(
                "Failed to open universal setup file {}, err: {}",
                setup_file_name,
                e
            )
        })?
    };
    Ok(BufReader::with_capacity(1 << 29, setup_file))
}

/// Returns universal setup in the monomial form of the given power of two (range: SETUP_MIN_POW2..=SETUP_MAX_POW2). Checks if file exists
pub fn get_universal_setup_monomial_form(
    power_of_two: u32,
) -> Result<Crs<Engine, CrsForMonomialForm>, anyhow::Error> {
    anyhow::ensure!(
        (SETUP_MIN_POW2..=SETUP_MAX_POW2).contains(&power_of_two),
        "setup power of two is not in the correct range"
    );
    let setup_file_name = format!("setup_2^{}.key", power_of_two);
    let mut buf_reader = get_universal_setup_file_buff_reader(&setup_file_name)?;
    Ok(Crs::<Engine, CrsForMonomialForm>::read(&mut buf_reader)
        .map_err(|e| format_err!("Failed to read Crs from setup file: {}", e))?)
}

/// Returns universal setup in lagrange form of the given power of two (range: SETUP_MIN_POW2..=SETUP_MAX_POW2). Checks if file exists
pub fn get_universal_setup_lagrange_form(
    power_of_two: u32,
) -> Result<Crs<Engine, CrsForLagrangeForm>, anyhow::Error> {
    anyhow::ensure!(
        (SETUP_MIN_POW2..=SETUP_MAX_POW2).contains(&power_of_two),
        "setup power of two is not in the correct range"
    );
    let setup_file_name = format!("setup_2^{}_lagrange.key", power_of_two);
    let mut buf_reader = get_universal_setup_file_buff_reader(&setup_file_name)?;
    Ok(Crs::<Engine, CrsForLagrangeForm>::read(&mut buf_reader)
        .map_err(|e| format_err!("Failed to read Crs from setup file: {}", e))?)
}

pub fn get_exodus_verification_key_path() -> PathBuf {
    let mut key = get_keys_root_dir();
    key.push("verification_exit.key");
    key
}

pub fn get_block_verification_key_path(block_chunks: usize) -> PathBuf {
    let mut key = get_keys_root_dir();
    key.push(&format!("verification_block_{}.key", block_chunks));
    key
}

pub fn get_verifier_contract_key_path() -> PathBuf {
    let mut contract = get_keys_root_dir();
    contract.push("KeysWithPlonkVerifier.sol");
    contract
}
