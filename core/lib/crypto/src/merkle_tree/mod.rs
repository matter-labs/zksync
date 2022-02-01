pub mod hasher;
pub mod parallel_smt;
pub mod rescue_hasher;
#[cfg(test)]
mod tests;

/// Sparse merkle tree used to calculate root hashes for the state in zkSync network.
pub type SparseMerkleTree<T, H, HH> = parallel_smt::SparseMerkleTree<T, H, HH>;
/// Default hasher used in the zkSync network for state hash calculations.
pub type RescueHasher<T> = rescue_hasher::RescueHasher<T>;

/// Represents the amount of RAM consumed by the tree.
/// Only data allocated on the heap is counted.
///
/// Field represent the amount of memory actually requested by the system.
/// For example, Rust `Vec`s allocate 2x previous amount on resize, so the `Vec` can
/// request up to 2x the amount of memory than is needed to fit all the elements.
///
/// All the fields represent the memory amount in bytes.
#[derive(Debug, Clone, Copy)]
pub struct TreeMemoryUsage {
    /// Memory used to store actual values in the tree.
    pub items: usize,
    /// Memory used to store hash nodes in the tree.
    pub nodes: usize,
    /// Memory used to store pre-calculated hashes for the "default" nodes.
    pub prehashed: usize,
    /// Memory used to store cache of calculated hashes for all the nodes in the tree.
    pub cache: usize,
    /// Total memory allocated by containers in the tree.
    pub allocated_total: usize,
}
