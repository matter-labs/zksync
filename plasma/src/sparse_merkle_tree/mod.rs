// Sparse Merkle tree with flexible hashing strategy

pub mod hasher;
pub mod pedersen_hasher;

use std::collections::HashMap;
use self::hasher::Hasher;
use super::primitives::IntoBits;

// 0 .. (N - 1)
type ItemIndex = usize;

// 0 .. (TREE_DEPTH - 1)
type Depth = usize;

// 1 .. (2 ^ TREE_DEPTH) - 1
// Merkle root has index 1
type HashIndex = usize;

#[derive(Debug, Clone)]
pub struct SparseMerkleTree<T: IntoBits + Default, Hash: Clone, H: Hasher<Hash>>
{
    tree_depth: Depth,
    prehashed: Vec<Hash>,
    items: HashMap<ItemIndex, T>,
    hashes: HashMap<HashIndex, Hash>,
    hasher: H,
}

impl<T, Hash, H> SparseMerkleTree<T, Hash, H>
    where T: IntoBits + Default,
          Hash: Clone,
          H: Hasher<Hash> + Default,
{

    pub fn new(tree_depth: Depth) -> Self {
        assert!(tree_depth > 1);
        let hasher = H::default();
        let items = HashMap::new();
        let hashes = HashMap::new();
        let mut prehashed = Vec::with_capacity(tree_depth-1);
        let mut cur = hasher.hash_bits(T::default().into_bits());
        prehashed.push(cur.clone());
        for i in 0..tree_depth-1 {
            cur = hasher.compress(&cur, &cur, i);
            prehashed.push(cur.clone());
        }
        prehashed.reverse();
        Self{tree_depth, prehashed, items, hashes, hasher}
    }

    fn depth(index: HashIndex) -> Depth {
        assert!(index > 0);
        let mut level: Depth = 0;
        let mut i = index;
        while i > 1 {
            level += 1;
            i >>= 1;
        }
        level
    }

    // How many iterms can the tree hold
    pub fn capacity(&self) -> usize {
        1 << (self.tree_depth - 1)
    }

    // How many hashes can the tree hold
    pub fn hash_capacity(&self) -> usize {
        (1 << self.tree_depth) - 1
    }

    pub fn insert(&mut self, index: ItemIndex, item: T) {
        assert!(index < self.capacity());

        let hash_index = (1 << self.tree_depth-1) + index;
        let hash = self.hasher.hash_bits(item.into_bits());
        //println!("index = {}, hash_index = {}", index, hash_index);
        self.hashes.insert(hash_index, hash);

        self.items.insert(index, item);

        let mut i = hash_index >> 1;
        while i > 0 {
            self.update_hash(i);
            i >>= 1;
        }
    }

    fn update_hash(&mut self, index: HashIndex) -> Hash {
        assert!(index > 0 && index <= self.hash_capacity());

        // indices for child nodes in the tree
        let lhs = index * 2;
        let rhs = index * 2 + 1;

        // if both child nodes are empty, use precomputed hash
        if !self.hashes.contains_key(&lhs) && !self.hashes.contains_key(&rhs) {
            return self.prehashed.get(Self::depth(index)).unwrap().clone()
        }

        let i = (self.tree_depth - 2) - Self::depth(index);
        let hash = self.hasher.compress(&self.get_hash(lhs), &self.get_hash(rhs), i);
        self.hashes.insert(index, hash.clone());
        hash
    }

    fn get_hash(&self, index: HashIndex) -> Hash {
        assert!(index > 0 && index <= self.hash_capacity());
        if let Some(hash) = self.hashes.get(&index) {
            // if hash for this index exists, return it
            hash.clone()
        } else {
            // otherwise return pre-computed
            self.prehashed.get(Self::depth(index)).unwrap().clone()
        }
    }

    pub fn merkle_path(&self, index: ItemIndex) -> Vec<(Hash, bool)> {
        assert!(index < self.capacity());
        let item_hash_index = (1 << self.tree_depth-1) + index;
        (0..(self.tree_depth-1)).map(|level| {
            let dir = item_hash_index & (1 << level) > 0;
            let hash_index = (item_hash_index >> level) ^ 1;
            let hash = self.get_hash(hash_index);
            (hash, dir)
        }).collect()
    }

    pub fn root_hash(&self) -> Hash {
        self.get_hash(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestHasher {}

    impl IntoBits for u64 {
        fn into_bits(&self) -> Vec<bool> {
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
            let r = 11 * lhs + 17 * rhs + 1;
            r
        }

    }

    type TestSMT = SparseMerkleTree<u64, u64, TestHasher>;

    #[test]
    fn test_merkle_tree_props() {
        assert_eq!(TestSMT::depth(1), 0);
        assert_eq!(TestSMT::depth(2), 1);
        assert_eq!(TestSMT::depth(3), 1);
        assert_eq!(TestSMT::depth(4), 2);
    }

    #[test]
    fn test_merkle_tree_insert() {
        let mut tree = TestSMT::new(3);

        assert_eq!(tree.capacity(), 4);
        assert_eq!(tree.hash_capacity(), 7);

        tree.insert(0, 1);
        //println!("{:?}", tree);
        assert_eq!(tree.root_hash(), 23707677);

        tree.insert(3, 2);
        //println!("{:?}", tree);
        assert_eq!(tree.root_hash(), 28442653);
    }

    #[test]
    fn test_merkle_path() {
        let mut tree = TestSMT::new(4);
        tree.insert(2, 1);
        let path = tree.merkle_path(2);
        assert_eq!(path, [(32768, false), (917505, true), (25690141, false)]);
    }
}
