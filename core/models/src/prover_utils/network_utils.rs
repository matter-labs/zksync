use super::{SETUP_MAX_POW2, SETUP_MIN_POW2};
use crate::node::Engine;
use crypto_exports::bellman::kate_commitment::{Crs, CrsForMonomialForm};
use failure::format_err;

/// Downloads universal setup in the monomial form of the given power of two (range: SETUP_MIN_POW2..=SETUP_MAX_POW2)
pub fn get_universal_setup_monomial_form(
    power_of_two: u32,
) -> Result<Crs<Engine, CrsForMonomialForm>, failure::Error> {
    failure::ensure!(
        (SETUP_MIN_POW2..=SETUP_MAX_POW2).contains(&power_of_two),
        "setup power of two is not in the correct range"
    );

    let setup_network_dir = std::env::var("PROVER_SETUP_NETWORK_DIR")?;
    let setup_dl_path = format!("{}/setup_2%5E{}.key", setup_network_dir, power_of_two);

    println!("Downloading universal setup from {}", &setup_dl_path);
    eprintln!("Downloading universal setup from {}", &setup_dl_path);

    let mut response_reader = reqwest::blocking::get(&setup_dl_path)?;

    Ok(
        Crs::<Engine, CrsForMonomialForm>::read(&mut response_reader)
            .map_err(|e| format_err!("Failed to read Crs from remote setup file: {}", e))?,
    )
}
