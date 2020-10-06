pub mod hasher;
pub mod parallel_smt;
pub mod rescue_hasher;
pub mod sequential_smt;
#[cfg(test)]
mod tests;

/// Sparse merkle tree used to calculate root hashes for the state in zkSync network.
pub type SparseMerkleTree<T, H, HH> = parallel_smt::SparseMerkleTree<T, H, HH>;
/// Default hasher used in the zkSync network for state hash calculations.
pub type RescueHasher<T> = rescue_hasher::RescueHasher<T>;

// TODO: return the code below and uncomment asserts

// pub fn verify_proof<E: Account>(&self, index: u32, item: Account, proof: Vec<(E::Fr, bool)>) -> bool {
//     use crate::sparse_merkle_tree::hasher::Hasher;

//     assert!(index < self.capacity());
//     let item_bits = item.get_bits_le();
//     let mut hash = self.hasher.hash_bits(item_bits);
//     let mut proof_index: u32 = 0;

//     for (i, e) in proof.clone().into_iter().enumerate() {
//         if e.1 {
//             // current is right
//             proof_index |= 1 << i;
//             hash = self.hasher.compress(&e.0, &hash, i);
//         } else {
//             // current is left
//             hash = self.hasher.compress(&hash, &e.0, i);
//         }
//         // print!("This level hash is {}\n", hash);
//     }

//     if proof_index != index {
//         return false;
//     }

//     hash == self.root_hash()
// }
