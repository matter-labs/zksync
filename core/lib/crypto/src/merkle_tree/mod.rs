pub mod hasher;
pub mod parallel_smt;
pub mod rescue_hasher;
#[cfg(test)]
mod tests;

/// Sparse merkle tree used to calculate root hashes for the state in zkSync network.
pub type SparseMerkleTree<T, H, HH> = parallel_smt::SparseMerkleTree<T, H, HH>;
/// Default hasher used in the zkSync network for state hash calculations.
pub type RescueHasher<T> = rescue_hasher::RescueHasher<T>;
