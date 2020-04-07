/// Sparse Merkle tree with flexible hashing strategy
use super::hasher::Hasher;
use crate::primitives::GetBits;
use std::collections::HashMap;
use std::fmt::Debug;

// Tree of depth 0 should contain ONE element that is also a root
// Tree of depth 1 should contain TWO elements
// Tree of depth 20 should contain 2^20 elements

// [0, (2^TREE_DEPTH - 1)]
type ItemIndex = u32;

// [0, TREE_DEPTH]
type Depth = u32;

// Hash index determines on what level of the tree the hash is
// and kept as level (where zero is a root) and item in a level indexed from 0
type HashIndex = (u32, u32);

type ItemIndexPacked = u64;

trait PackToIndex {
    fn pack(&self) -> ItemIndexPacked;
}

impl PackToIndex for HashIndex {
    fn pack(&self) -> ItemIndexPacked {
        let mut packed = 0u64;
        packed += u64::from(self.0);
        packed <<= 32u64;
        packed += u64::from(self.1);

        packed
    }
}

/// Sparse Merkle tree is basically a [Merkle tree] which is allowed to have
/// gapes between elements.
///
/// The essential operation of this structure is obtaining a root hash of the structure,
/// which represents the state of all of the tree elements.
///
/// The sparseness of the tree is implementing through a "default leaf" - an item which
/// hash will be used for the missing indices instead of the actual element hash.
///
/// Since this means that basically the tree is "full" all the time (all the empty indices
/// are taken by the "default" element), the tree has fixed capacity and cannot be extended
/// above that. The root hash is calculated for the full tree every time.
///
/// [Merkle tree]: https://en.wikipedia.org/wiki/Merkle_tree
#[derive(Debug, Clone)]
pub struct SparseMerkleTree<T: GetBits, Hash: Clone + Eq + Debug, H: Hasher<Hash>> {
    tree_depth: Depth,
    pub prehashed: Vec<Hash>,
    pub items: HashMap<ItemIndex, T>,
    pub hashes: HashMap<ItemIndexPacked, Hash>,
    pub hasher: H,
}

impl<T, Hash, H> SparseMerkleTree<T, Hash, H>
where
    T: GetBits,
    Hash: Clone + Eq + Debug,
    H: Hasher<Hash>,
{
    /// Returns the capacity of the tree (how many items can the tree hold).
    pub fn capacity(&self) -> u32 {
        1 << self.tree_depth
    }

    /// Inserts an element to the tree.
    pub fn insert(&mut self, index: ItemIndex, item: T) {
        assert!(index < self.capacity());

        self.recalculate_hashes(index, &item);

        self.items.insert(index, item);
    }

    /// Stores the item hash and updates hashes up to the tree root.
    fn recalculate_hashes(&mut self, index: ItemIndex, item: &T) {
        // Current hash index relates to the last tree layer and has
        // the same index as the tree item.
        let hash_index = (self.tree_depth, index);

        // We calculate a store the item hash in a location described above.
        let hash = self.hasher.hash_bits(item.get_bits_le());
        self.hashes.insert(hash_index.pack(), hash);

        // Now we have to go through all the level up to zero (the root layer)
        // and update hashes that were affected by this item.
        let mut next_level = (hash_index.0, hash_index.1);
        for _ in 0..next_level.0 {
            // The next level is one height closer to zero (which is a root height),
            // and has a two times smaller index (if the original index of the item is 4,
            // then the highest layer hash index is 4, then it's 2, then 1, and then finally it's 0).
            next_level = (next_level.0 - 1, next_level.1 >> 1);
            self.update_hash(next_level);
        }

        // After updating the hash we ensure that we've gone up to the tree rot.
        assert_eq!(next_level.0, 0);
    }

    // pub fn calculate_hash_index(& self, index: ItemIndex) -> HashIndex {
    //     let hash_index = (1 << self.tree_depth-1) + index;
    //     hash_index
    // }

    /// Calculates the hash of the non-bottom layer by aggregating two
    /// bottom-laying hashes. For layer 1 and hash index 0, the hashes
    /// 0 and 1 of layer 2 are aggregated.
    fn update_hash(&mut self, index: HashIndex) -> Hash {
        // should NEVER be used to update the leaf hash

        // print!("Updating index {}, {}\n", index.0, index.1);

        assert!(index.0 < self.tree_depth);
        assert!(index.1 < self.capacity());

        // Indices for child nodes in the tree: one hight up, and (x2) and (x2 + 1) indices
        // at the layer.
        let lhs_index = (index.0 + 1, (index.1 << 1));
        let rhs_index = (index.0 + 1, (index.1 << 1) + 1);

        let lhs_hash = self.get_hash(lhs_index);
        let rhs_hash = self.get_hash(rhs_index);

        //let idx = (1 << index.0) + index.1;
        //debug!("({:?}, {:?}, {})", &lhs_hash, &rhs_hash, (self.tree_depth - 1 - index.0));

        let hash = self.hasher.compress(
            &lhs_hash,
            &rhs_hash,
            (self.tree_depth - 1 - index.0) as usize,
        );

        //debug!("hash [{}] = {:?}", (1 << index.0) + index.1, hash);

        self.hashes.insert(index.pack(), hash.clone());
        hash
    }

    /// Returns the hash of the element with a given index.
    pub fn get_hash(&self, index: HashIndex) -> Hash {
        // print!("Reading hash for index {}, {}\n", index.0, index.1);

        assert!(index.0 <= self.tree_depth);
        assert!(index.1 < self.capacity());

        if let Some(hash) = self.hashes.get(&index.pack()) {
            // This is a non-default element, and there is a hash stored for it.

            // print!("Found non-default hash for index {}, {}\n", index.0, index.1);
            hash.clone()
        } else {
            // If there was no hash in the calculated hashes table, it means that
            // the item with such an index is missing in the tree, and we must return
            // the "default" hash, which is a hash of the element chosen to be "default"
            // for this tree.

            // print!("Found default hash for index {}, {}\n", index.0, index.1);
            self.prehashed.get((index.0) as usize).unwrap().clone()
        }
    }

    /// Creates a proof of existence for a certain element of the tree.
    /// Returned value is a list of pairs, where the first element is
    /// the aggregated coupling hash for current layer, and the second is
    /// the direction.
    pub fn merkle_path(&self, index: ItemIndex) -> Vec<(Hash, bool)> {
        // print!("Making a proof for index {}\n", index);
        assert!(index < self.capacity());
        let mut hash_index = (self.tree_depth, index);

        (0..self.tree_depth)
            .rev()
            .map(|_level| {
                let dir = (hash_index.1 & 1) > 0;
                let proof_index = (hash_index.0, hash_index.1 ^ 1);
                let hash = self.get_hash(proof_index);
                hash_index = (hash_index.0 - 1, hash_index.1 >> 1);
                (hash, dir)
            })
            .collect()
    }

    // pub fn verify_proof(&self, index: ItemIndex, item: T, proof: Vec<(Hash, bool)>) -> bool {
    //     assert!(index < self.capacity());
    //     let item_bits = item.get_bits_le();
    //     let mut hash = self.hasher.hash_bits(item_bits);
    //     let mut proof_index: ItemIndex = 0;

    //     for (i, e) in proof.clone().into_iter().enumerate() {
    //         if e.1 {
    //             // current is right
    //             proof_index |= 1 << i;
    //             hash = self.hasher.compress(&e.0, &hash, i);
    //         } else {
    //             // current is left
    //             hash = self.hasher.compress(&hash, &e.0, i);
    //         }
    //     }

    //     if proof_index != index {
    //         return false;
    //     }

    //     hash == self.root_hash()
    // }

    /// Returns the Merkle root hash of the tree. This operation is O(1).
    pub fn root_hash(&self) -> Hash {
        // Root hash is stored at layer 0 and index 0.
        self.get_hash((0, 0))
    }
}

impl<T, Hash, H> SparseMerkleTree<T, Hash, H>
where
    T: GetBits + Default,
    Hash: Clone + Eq + Debug,
    H: Hasher<Hash> + Default,
{
    /// Creates a new tree of certain depth (which determines the
    /// capacity of the tree, since the given height will not be
    /// exceeded).
    pub fn new(tree_depth: Depth) -> Self {
        Self::new_with_leaf(tree_depth, T::default())
    }

    /// Obtains the element for a certain index.
    pub fn get(&self, index: ItemIndex) -> Option<&T> {
        self.items.get(&index)
    }

    /// Removes an element with a given index, and returns the removed
    /// element (if it existed in the tree).
    pub fn remove(&mut self, index: ItemIndex) -> Option<T> {
        let old = self.items.remove(&index);
        let item = T::default();

        self.insert(index, item);

        old
    }

    /// Removes an element with a given index. Does nothing if there was
    /// no element at the provided index.
    pub fn delete(&mut self, index: ItemIndex) {
        self.remove(index);
    }
}

impl<T, Hash, H> SparseMerkleTree<T, Hash, H>
where
    T: GetBits,
    Hash: Clone + Eq + Debug,
    H: Hasher<Hash> + Default,
{
    /// Creates a new tree with the default item provided.
    /// This method is similar to `SparseMerkleTree::new`, but does not rely
    /// on the `Default` trait implementation for `T`.
    pub fn new_with_leaf(tree_depth: Depth, default_leaf: T) -> Self {
        let hasher = H::default();

        // we need to make sparse hashes for tree depth levels
        let mut prehashed = Vec::with_capacity((tree_depth + 1) as usize);
        let mut cur = hasher.hash_bits(default_leaf.get_bits_le());
        prehashed.push(cur.clone());

        for i in 0..tree_depth {
            cur = hasher.compress(&cur, &cur, i as usize);
            prehashed.push(cur.clone());
        }
        prehashed.reverse();

        // print!("Made default hashes in quantity {}\n", prehashed.len());

        assert_eq!(prehashed.len() - 1, tree_depth as usize);
        Self {
            tree_depth,
            prehashed,
            items: HashMap::new(),
            hashes: HashMap::new(),
            hasher,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A very simple and fast hasher.
    /// Since it uses an `u64` multiplication, it can easily overflow
    /// when used in big trees, so try to use it for small trees only.
    #[derive(Debug)]
    struct TestHasher;

    impl Default for TestHasher {
        fn default() -> Self {
            Self {}
        }
    }

    impl Hasher<u64> for TestHasher {
        fn hash_bits<I: IntoIterator<Item = bool>>(&self, value: I) -> u64 {
            let mut acc = 0;
            let v: Vec<bool> = value.into_iter().collect();
            for i in v.iter() {
                acc <<= 1;
                if *i {
                    acc |= 1
                };
            }
            acc
        }

        fn hash_elements<I: IntoIterator<Item = u64>>(&self, _elements: I) -> u64 {
            unimplemented!()
        }

        fn compress(&self, lhs: &u64, rhs: &u64, i: usize) -> u64 {
            11 * lhs + 17 * rhs + 1 + i as u64
            //log::debug!("compress {} {}, {} => {}", lhs, rhs, i, r);
        }
    }

    type TestSMT = SparseMerkleTree<u64, u64, TestHasher>;

    //     #[test]
    //     fn test_merkle_tree_props() {
    //         let mut tree = TestSMT::new(3);
    //         assert_eq!(TestSMT::depth(1), 0);
    //         assert_eq!(TestSMT::depth(2), 1);
    //         assert_eq!(TestSMT::depth(3), 1);
    //         assert_eq!(TestSMT::depth(4), 2);
    //     }

    #[test]
    fn test_merkle_tree_insert() {
        let mut tree = TestSMT::new(3);

        assert_eq!(tree.capacity(), 8);

        tree.insert(0, 1);
        log::debug!("{:?}", tree);
        assert_eq!(tree.root_hash(), 697_516_875);

        tree.insert(0, 2);
        log::debug!("{:?}", tree);
        assert_eq!(tree.root_hash(), 741_131_083);

        tree.insert(3, 2);
        //log::debug!("{:?}", tree);
        assert_eq!(tree.root_hash(), 793_215_819);
    }

    #[test]
    fn test_merkle_path() {
        let mut tree = TestSMT::new(3);
        tree.insert(2, 1);
        let path = tree.merkle_path(2);
        //log::debug!("{:?}", tree);
        assert_eq!(path, [(32768, false), (917_505, true), (25_690_142, false)]);
    }

    /// Performs some basic insert/remove operations.
    #[test]
    fn merkle_tree_workflow() {
        let mut tree = TestSMT::new(3);

        // Add one element with known-before hash.
        tree.insert(0, 1);
        assert_eq!(tree.root_hash(), 697_516_875);

        // Add more elements.
        for idx in 1..8 {
            tree.insert(idx, idx as u64);
        }

        // Remove them (and check that within removing we can obtain them).
        for idx in (1..8).rev() {
            assert_eq!(tree.remove(idx), Some(idx as u64));
        }

        // The first element left only, hash should be the same as in the beginning.
        assert_eq!(tree.root_hash(), 697_516_875);
    }

    /// A very simple benchmark for tree.
    ///
    /// This is a self-made benchmark. It is disabled by default, since it's a long-runner,
    /// but in can be run as follows:
    ///
    /// ```text
    /// f cargo test tree_bench --features=run_benches -- --nocapture
    /// ```
    ///
    /// Note that you have to run it in the `models` directory, as cargo does not support
    /// `--features` flag in virtual workspaces.
    #[test]
    #[cfg_attr(not(feature = "run_benches"), ignore)]
    fn tree_bench() {
        // Since this benchmark is temporary and will likely be replaced with a
        // `criterion` benches or something similar, uses and internal functions
        // are placed within this test to not interfere with other tests.
        use std::time;

        use crate::circuit::account::CircuitAccount;
        use crate::franklin_crypto::bellman::pairing::bn256::{Bn256, Fr};
        use crate::merkle_tree::PedersenHasher;

        /// A simple bench stats counter.
        #[derive(Debug, Default)]
        struct BenchStats {
            min_time: Option<time::Duration>,
            max_time: Option<time::Duration>,
            overall: time::Duration,
            n_samples: u32,
        }

        impl BenchStats {
            pub fn add_time(&mut self, time: time::Duration) {
                if self.min_time.unwrap_or(time) >= time {
                    self.min_time = Some(time);
                }
                if self.max_time.unwrap_or(time) <= time {
                    self.max_time = Some(time);
                }

                self.overall += time;
                self.n_samples += 1;
            }

            pub fn average(&mut self) -> time::Duration {
                self.overall / self.n_samples
            }
        }

        /// Function to generate an unique account.
        fn gen_account(id: u64) -> CircuitAccount<Bn256> {
            let mut account = CircuitAccount::<Bn256>::default();

            let id_hex = format!("{:064x}", id);
            account.address = Fr::from_hex(id_hex.as_ref()).unwrap();

            account
        }

        // In this bench we want "real-life" test, so we use the actual type of
        // the SMT.
        type RealSMT = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;

        let mut create_stats = BenchStats::default();
        let mut insert_stats = BenchStats::default();

        // Create SMT.
        let start = time::Instant::now();
        let mut tree = RealSMT::new(crate::params::account_tree_depth() as u32);
        let end = time::Instant::now();
        println!("RealSMT creation time: {}ms", (end - start).as_millis());

        // Add several accounts to it and measure the account generation and
        // the insertion times.
        // We measure account generation time, since it contains a SMT inside and
        // since it's a part of the actual accounts SMT interaction, we want to
        // be sure that it's a fast operation.
        let n_samples = 10;
        for id in 0..n_samples {
            let start = time::Instant::now();
            let account = gen_account(id);
            let end = time::Instant::now();
            create_stats.add_time(end - start);

            let start = time::Instant::now();
            tree.insert(id as u32, account);
            let end = time::Instant::now();
            insert_stats.add_time(end - start);
        }

        println!("Bench stats <CREATE> ({} samples):", n_samples);
        println!("Min time: {}ms", create_stats.min_time.unwrap().as_millis());
        println!("Max time: {}ms", create_stats.max_time.unwrap().as_millis());
        println!("Avg time: {}ms", create_stats.average().as_millis());

        println!("Bench stats <INSERT> ({} samples):", n_samples);
        println!("Min time: {}ms", insert_stats.min_time.unwrap().as_millis());
        println!("Max time: {}ms", insert_stats.max_time.unwrap().as_millis());
        println!("Avg time: {}ms", insert_stats.average().as_millis());
    }
}
