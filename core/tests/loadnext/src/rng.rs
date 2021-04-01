//! Seedable RNG capable of creating reproducible test scenarios.

pub trait LoadtestRNG {
    fn seed(&mut self, seed: u64);

    fn rand_int(&mut self) -> u64;
}
