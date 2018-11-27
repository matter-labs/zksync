// Sparse Merkle tree with flexible hashing strategy

use std::collections::HashMap;
use std::marker::Sized;

// 0 .. N-1
type ItemIndex = usize;

// 0 .. (TREE_DEPTH - 1)
type Depth = usize;

// 1 .. (2^TREE_DEPTH) - 1: Merkle root has index 1
type HashIndex = usize;

pub trait Hasher<T> {
    type Hash: Copy;

    fn hash(value: T) -> Self::Hash;
    fn compress(lhs: Self::Hash, rhs: Self::Hash) -> Self::Hash;
    fn empty_hash() -> Self::Hash;
}

#[derive(Debug, Clone)]
pub struct SparseMerkleTree<T, H: Hasher<T>>
{
    tree_depth: Depth,
    prehashed: Vec<H::Hash>,
    items: HashMap<ItemIndex, T>,
    hashes: HashMap<HashIndex, H::Hash>,
}

#[derive(Debug)]
struct U64Hasher {}

impl Hasher<u64> for U64Hasher {
    type Hash = u64;

    fn hash(value: u64) -> Self::Hash {
        value * 7
    }

    fn compress(lhs: Self::Hash, rhs: Self::Hash) -> Self::Hash {
        11 * lhs + 17 * rhs + 1
    }

    fn empty_hash() -> Self::Hash {
        0
    }
}

impl<T, H: Hasher<T>> SparseMerkleTree<T, H> {

    fn new(depth: Depth) -> Self {
        assert!(depth > 1);
        let items = HashMap::new();
        let hashes = HashMap::new();
        let mut prehashed = Vec::with_capacity(depth-1);
        let mut cur = H::empty_hash();
        for _ in 0..depth {
            cur = H::compress(cur, cur);
            prehashed.push(cur);
        }
        prehashed.reverse();
        Self{ tree_depth: depth, prehashed, items, hashes}
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

    fn capacity(&self) -> usize {
        2 << self.tree_depth
    }

    fn insert(&mut self, index: ItemIndex, item: T) {
        self.items.insert(index, item);
    }

    fn get_hash(&self, index: HashIndex) -> H::Hash {
        assert!(index < self.capacity());

        // if hash for this index exists, return it
        if let Some(&hash) = self.hashes.get(&index) {
            return hash
        }

        // indices for child nodes in the tree
        let lhs = index * 2;
        let rhs = index * 2 + 1;

        // if both child nodes are empty, use precomputed hash
        if !self.hashes.contains_key(&lhs) && !self.hashes.contains_key(&rhs) {
            return *self.prehashed.get(Self::depth(index)).unwrap()
        }

        H::compress(self.get_hash(lhs), self.get_hash(rhs))
    }

    fn root_hash(&self) -> H::Hash {
        self.get_hash(1)
    }

}


#[test]
fn test_merkle_tree_depth() {
    assert_eq!(SparseMerkleTree::<u64, U64Hasher>::depth(1), 0);
    assert_eq!(SparseMerkleTree::<u64, U64Hasher>::depth(2), 1);
    assert_eq!(SparseMerkleTree::<u64, U64Hasher>::depth(3), 1);
    assert_eq!(SparseMerkleTree::<u64, U64Hasher>::depth(4), 2);
}

#[test]
fn test_merkle_tree_insert() {
    let mut tree = SparseMerkleTree::<u64, U64Hasher>::new(3);
    tree.insert(0, 1);
    println!("{:?}", tree);
    assert_eq!(tree.root_hash(), 813);
}