// Sparse Merkle tree with flexible hashing strategy

pub mod hasher;
pub mod pedersen_hasher;

use std::fmt::Debug;
use std::collections::HashMap;
use self::hasher::Hasher;
use super::primitives::GetBits;
use ff::{PrimeField};

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

#[derive(Debug, Clone)]
pub struct SparseMerkleTree<T: GetBits + Default, Hash: Clone + Eq + Debug, H: Hasher<Hash>>
{
    tree_depth: Depth,
    pub prehashed: Vec<Hash>,
    pub items: HashMap<ItemIndex, T>,
    pub hashes: HashMap<ItemIndexPacked, Hash>,
    pub hasher: H,
}

impl<T, Hash, H> SparseMerkleTree<T, Hash, H>
    where T: GetBits + Default,
          Hash: Clone + Eq + Debug,
          H: Hasher<Hash> + Default,
{

    pub fn new(tree_depth: Depth) -> Self {
        let hasher = H::default();
        let items = HashMap::new();
        let hashes = HashMap::new();
        // we need to make sparse hashes for tree depth levels
        let mut prehashed = Vec::with_capacity((tree_depth + 1) as usize);
        let mut cur = hasher.hash_bits(T::default().get_bits_le());
        prehashed.push(cur.clone());

        for i in 0..tree_depth {
            cur = hasher.compress(&cur, &cur, i as usize);
            prehashed.push(cur.clone());
        }
        prehashed.reverse();

        // print!("Made default hashes in quantity {}\n", prehashed.len());

        assert_eq!(prehashed.len() - 1, tree_depth as usize);
        Self{tree_depth, prehashed, items, hashes, hasher}
    }

    // How many items can the tree hold
    pub fn capacity(&self) -> u32 {
        1 << self.tree_depth
    }

    pub fn insert(&mut self, index: ItemIndex, item: T) {
        assert!(index < self.capacity());
        let hash_index = (self.tree_depth, index);

        let item_bits = item.get_bits_le();
        // for b in item_bits.clone() {
        //     if b {
        //         print!("1");
        //     } else {
        //         print!("0");
        //     }
        // }
        // print!("\n");
        let hash = self.hasher.hash_bits(item_bits);
        // print!("Inserting at index {}\n", index);
        // print!("Packed into index {}, {}\n", hash_index.0, hash_index.1);

        //println!("hash [{}] = {:?}", (1 << hash_index.0) + hash_index.1, &hash);

        self.hashes.insert(hash_index.pack(), hash);

        self.items.insert(index, item);

        let mut next_level = (hash_index.0, hash_index.1);

        // print!("Have updated index {}, {}, will cascade\n", next_level.0, next_level.1);
        for _ in 0..next_level.0 {
            next_level = (next_level.0 - 1, next_level.1 >> 1);
            self.update_hash(next_level);

        }
        // print!("Have updated up to {}, {}\n", next_level.0, next_level.1);
        assert_eq!(next_level.0, 0);
    }

    // pub fn calculate_hash_index(& self, index: ItemIndex) -> HashIndex {
    //     let hash_index = (1 << self.tree_depth-1) + index;
    //     hash_index
    // }

    fn update_hash(&mut self, index: HashIndex) -> Hash {
        // should NEVER be used to update the leaf hash

        // print!("Updating index {}, {}\n", index.0, index.1);

        assert!(index.0 < self.tree_depth);
        assert!(index.1 < self.capacity());

        // indices for child nodes in the tree
        let lhs_index = (index.0 + 1, (index.1 << 1));
        let rhs_index = (index.0 + 1, (index.1 << 1) + 1);

        let lhs_hash = self.get_hash(lhs_index);
        let rhs_hash = self.get_hash(rhs_index);

        //let idx = (1 << index.0) + index.1;
        //println!("({:?}, {:?}, {})", &lhs_hash, &rhs_hash, (self.tree_depth - 1 - index.0));

        let hash = self.hasher.compress(&lhs_hash, &rhs_hash, (self.tree_depth - 1 - index.0) as usize);

        //println!("hash [{}] = {:?}", (1 << index.0) + index.1, hash);

        self.hashes.insert(index.pack(), hash.clone());
        hash

    }

    pub fn get_hash(&self, index: HashIndex) -> Hash {
        // print!("Reading hash for index {}, {}\n", index.0, index.1);

        assert!(index.0 <= self.tree_depth);
        assert!(index.1 < self.capacity());

        if let Some(hash) = self.hashes.get(&index.pack()) {
            // if hash for this index exists, return it
            // print!("Found non-default hash for index {}, {}\n", index.0, index.1);
            hash.clone()
        } else {
            // otherwise return pre-computed
            // print!("Found default hash for index {}, {}\n", index.0, index.1);
            self.prehashed.get((index.0) as usize).unwrap().clone()
        }
    }

    pub fn merkle_path(&self, index: ItemIndex) -> Vec<(Hash, bool)> {
        // print!("Making a proof for index {}\n", index);
        assert!(index < self.capacity());
        let mut hash_index = (self.tree_depth, index);

        (0..self.tree_depth).rev().map(|level| {
            let dir = (hash_index.1 & 1) > 0;
            let proof_index = (hash_index.0, hash_index.1 ^ 1);
            let hash = self.get_hash(proof_index);
            hash_index = (hash_index.0 - 1, hash_index.1 >> 1);
            (hash, dir)
        }).collect()
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

    pub fn root_hash(&self) -> Hash {
        self.get_hash((0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestHasher {}

    impl GetBits for u64 {
        fn get_bits_le(&self) -> Vec<bool> {
            let mut acc = Vec::new();
            let mut i = *self + 1;
            for _ in 0..16 {
                acc.push(i & 1 == 1);
                i >>= 1;
            }
            acc
        }
    }

    impl Default for TestHasher {
        fn default() -> Self { Self {} }
    }

    impl Hasher<u64> for TestHasher {

        fn hash_bits<I: IntoIterator<Item=bool>>(&self, value: I) -> u64 {
            let mut acc = 0;
            let v: Vec<bool> = value.into_iter().collect();
            for i in v.iter() {
                acc <<= 1;
                if *i {acc |= 1};
            }
            acc
        }

        fn compress(&self, lhs: &u64, rhs: &u64, i: usize) -> u64 {
            let r = 11 * lhs + 17 * rhs + 1 + i as u64;
            //println!("compress {} {}, {} => {}", lhs, rhs, i, r);
            r
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
        println!("{:?}", tree);
        assert_eq!(tree.root_hash(), 697516875);

        tree.insert(0, 2);
        println!("{:?}", tree);
        assert_eq!(tree.root_hash(), 741131083);

        tree.insert(3, 2);
        //println!("{:?}", tree);
        assert_eq!(tree.root_hash(), 793215819);
    }

    #[test]
    fn test_merkle_path() {
        let mut tree = TestSMT::new(3);
        tree.insert(2, 1);
        let path = tree.merkle_path(2);
        //println!("{:?}", tree);
        assert_eq!(path, [(32768, false), (917505, true), (25690142, false)]);
    }
}
