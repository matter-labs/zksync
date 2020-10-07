use super::{SETUP_MAX_POW2, SETUP_MIN_POW2};
use anyhow::format_err;
use backoff::Operation;
use std::time::Duration;
use zksync_crypto::bellman::kate_commitment::{Crs, CrsForMonomialForm};
use zksync_crypto::Engine;

/// Downloads universal setup in the monomial form of the given power of two (range: SETUP_MIN_POW2..=SETUP_MAX_POW2)
pub fn get_universal_setup_monomial_form(
    power_of_two: u32,
) -> Result<Crs<Engine, CrsForMonomialForm>, anyhow::Error> {
    anyhow::ensure!(
        (SETUP_MIN_POW2..=SETUP_MAX_POW2).contains(&power_of_two),
        "setup power of two is not in the correct range"
    );

    let mut retry_op = move || try_to_download_setup(power_of_two);

    retry_op
        .retry_notify(&mut get_backoff(), |err, next_after: Duration| {
            let duration_secs = next_after.as_millis() as f32 / 1000.0f32;

            log::warn!(
                "Failed to download setup err: <{}>, retrying after: {:.1}s",
                err,
                duration_secs,
            )
        })
        .map_err(|e| {
            format_err!(
                "Can't download setup, max elapsed time of the backoff reached: {}",
                e
            )
        })
}

fn try_to_download_setup(
    power_of_two: u32,
) -> Result<Crs<Engine, CrsForMonomialForm>, backoff::Error<anyhow::Error>> {
    let setup_network_dir = std::env::var("PROVER_SETUP_NETWORK_DIR")
        .map_err(|e| backoff::Error::Permanent(e.into()))?;

    let setup_dl_path = format!("{}/setup_2%5E{}.key", setup_network_dir, power_of_two);

    log::info!("Downloading universal setup from {}", &setup_dl_path);

    let mut response_reader =
        reqwest::blocking::get(&setup_dl_path).map_err(|e| backoff::Error::Transient(e.into()))?;

    Crs::<Engine, CrsForMonomialForm>::read(&mut response_reader)
        .map_err(|e| format_err!("Failed to read Crs from remote setup file: {}", e))
        .map_err(backoff::Error::Transient)
}

fn get_backoff() -> backoff::ExponentialBackoff {
    let mut backoff = backoff::ExponentialBackoff::default();
    backoff.current_interval = Duration::from_secs(5);
    backoff.initial_interval = Duration::from_secs(5);
    backoff.multiplier = 1.2f64;
    backoff.max_interval = Duration::from_secs(80);
    backoff.max_elapsed_time = Some(Duration::from_secs(10 * 60));
    backoff
}
