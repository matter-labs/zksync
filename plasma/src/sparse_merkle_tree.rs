// Sparse Merkle tree with flexible hashing strategy

use std::collections::HashMap;
use std::marker::PhantomData;
use super::hasher::{Hasher, Factory};

// 0 .. N-1
type ItemIndex = usize;

// 0 .. (TREE_DEPTH - 1)
type Depth = usize;

// 1 .. (2^TREE_DEPTH) - 1: Merkle root has index 1
type HashIndex = usize;

//pub trait Hasher<T> {
//    type Hash: Clone;
//    fn hash(value: &T) -> Self::Hash;
//    fn compress(lhs: &Self::Hash, rhs: &Self::Hash) -> Self::Hash;
//    fn empty_hash() -> Self::Hash;
//}

#[derive(Debug, Clone)]
pub struct SparseMerkleTree<T, Hash: Clone, H: Hasher<T, Hash>>
{
    tree_depth: Depth,
    prehashed: Vec<Hash>,
    items: HashMap<ItemIndex, T>,
    hashes: HashMap<HashIndex, Hash>,
    hasher: H,
}

impl<T, Hash, H> SparseMerkleTree<T, Hash, H>
    where Hash: Clone,
          H: Hasher<T, Hash> + Factory,
{

    pub fn new(tree_depth: Depth) -> Self {
        assert!(tree_depth > 1);
        let hasher = H::new();
        let items = HashMap::new();
        let hashes = HashMap::new();
        let mut prehashed = Vec::with_capacity(tree_depth-1);
        let mut cur = hasher.empty_hash();
        prehashed.push(cur.clone());
        for _ in 0..tree_depth-1 {
            cur = hasher.compress(&cur, &cur);
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

    pub fn capacity(&self) -> usize {
        2 << self.tree_depth
    }

    pub fn insert(&mut self, index: ItemIndex, item: T) {
        self.items.insert(index, item);
        let mut i = index;
        while i > 0 {
            self.get_hash(i);
            i >>= 1;
        }
    }

    fn get_hash(&self, index: HashIndex) -> Hash {
        assert!(index < self.capacity());

        // if hash for this index exists, return it
        if let Some(hash) = self.hashes.get(&index) {
            return hash.clone()
        }

        // TODO: if bottom, get hash of actual item

        // indices for child nodes in the tree
        let lhs = index * 2;
        let rhs = index * 2 + 1;

        // if both child nodes are empty, use precomputed hash
        if !self.hashes.contains_key(&lhs) && !self.hashes.contains_key(&rhs) {
            return self.prehashed.get(Self::depth(index)).unwrap().clone()
        }

        self.hasher.compress(&self.get_hash(lhs), &self.get_hash(rhs))
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

    impl Factory for TestHasher {
        fn new() -> Self { Self {} }
    }

    impl Hasher<u64, u64> for TestHasher {
        fn hash(&self, value: &u64) -> u64 {
            value * 7
        }
        fn compress(&self, lhs: &u64, rhs: &u64) -> u64 {
            11 * lhs + 17 * rhs + 1
        }
        fn empty_hash(&self) -> u64 {
            7
        }
    }

    type TestSMT = SparseMerkleTree<u64, u64, TestHasher>;

    #[test]
    fn test_merkle_tree_depth() {
        assert_eq!(TestSMT::depth(1), 0);
        assert_eq!(TestSMT::depth(2), 1);
        assert_eq!(TestSMT::depth(3), 1);
        assert_eq!(TestSMT::depth(4), 2);
    }

    #[test]
    fn test_merkle_tree_insert() {
        let mut tree = TestSMT::new(3);
        tree.insert(0, 1);
        println!("{:?}", tree);
        assert_eq!(tree.root_hash(), 5517);
    }
}