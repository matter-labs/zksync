// Sparse Merkle tree with batch updates

use crate::hasher::Hasher;
use fnv::FnvHashMap;
use models::primitives::GetBits;
use std::fmt::Debug;

// use std::time::Duration;
// use rayon::prelude::*;

fn select<T>(condition: bool, a: T, b: T) -> (T, T) {
    if condition {
        (a, b)
    } else {
        (b, a)
    }
}

// Nodes enumarated starting with index(root) = 1
// We need 2 * TREE_HEIGHT bits
type NodeIndex = u64;

// Lead index: 0 <= i < N (u64 to avoid conversions; 64 bit HW should be used anyway)
type ItemIndex = usize;

// Tree of depth 0: 1 item (which is root), level 0 only
// Tree of depth 1: 2 items, levels 0 and 1
// Tree of depth N: 2 ^ N items, 0 <= level < depth
type Depth = usize;

// Index of the node in the vector; slightly inefficient, won't be needed when rust gets non-lexical timelines
type NodeRef = usize;

#[derive(Debug, Clone)]
pub struct Node {
    depth: Depth,
    index: NodeIndex,
    left: Option<NodeRef>,
    right: Option<NodeRef>,
}

#[derive(Clone)]
pub struct SparseMerkleTree<T, Hash, H>
where
    T: GetBits + Default + Sync,
    Hash: Clone + Debug + Sync + Send,
    H: Hasher<Hash> + Sync,
{
    pub items: FnvHashMap<ItemIndex, T>,

    prehashed: Vec<Hash>,
    tree_depth: Depth,
    hasher: H,

    // intermediate nodes
    root: NodeRef,
    nodes: Vec<Node>,
    cache: FnvHashMap<NodeIndex, Hash>,
}

impl<T, Hash, H> SparseMerkleTree<T, Hash, H>
where
    T: GetBits + Default + Sync,
    Hash: Clone + Debug + Sync + Send,
    H: Hasher<Hash> + Default + Sync,
{
    pub fn new(tree_depth: Depth) -> Self {
        assert!(tree_depth > 1);
        let hasher = H::default();
        let items = FnvHashMap::default();
        let mut nodes = Vec::new();
        nodes.push(Node {
            index: 1,
            depth: 0,
            left: None,
            right: None,
        });

        let mut prehashed = Vec::with_capacity(tree_depth);
        let mut cur = hasher.hash_bits(T::default().get_bits_le());
        prehashed.push(cur.clone());
        for i in 0..tree_depth {
            cur = hasher.compress(&cur, &cur, i);
            prehashed.push(cur.clone());
        }
        prehashed.reverse();

        let cache = FnvHashMap::default();

        Self {
            tree_depth,
            prehashed,
            items,
            hasher,
            nodes,
            cache,
            root: 0,
        }
    }

    #[inline(always)]
    fn depth(index: NodeIndex) -> Depth {
        let mut level: Depth = 0;
        let mut i = index;
        while i > 1 {
            level += 1;
            i >>= 1;
        }
        level
    }

    // How many items can the tree hold
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        1 << self.tree_depth
    }

    // How many hashes can the tree hold
    #[inline(always)]
    #[allow(dead_code)]
    fn nodes_capacity(&self) -> usize {
        (1 << (self.tree_depth + 1)) - 1
    }

    fn wipe_cache(&mut self, child: NodeIndex, parent: NodeIndex) {
        if self.cache.remove(&child).is_some() {
            let mut i = child >> 1;
            while i > parent {
                self.cache.remove(&i);
                i >>= 1;
            }
        }
    }

    pub fn insert(&mut self, item_index: ItemIndex, item: T) {
        assert!(item_index < self.capacity());
        let tree_depth = self.tree_depth;
        let leaf_index: NodeIndex = ((1 << tree_depth) + item_index) as NodeIndex;

        self.items.insert(item_index, item);

        // invalidate root cache
        self.cache.remove(&1);

        // traverse the tree
        let mut cur_ref = self.root;
        loop {
            let cur = { self.nodes[cur_ref].clone() };

            let dir = (leaf_index & (1 << (tree_depth - cur.depth - 1))) > 0;
            let link = if dir { cur.right } else { cur.left };
            if let Some(next_ref) = link {
                let next = self.nodes[next_ref].clone();
                let leaf_index_normalized = leaf_index >> (tree_depth - next.depth);

                if leaf_index_normalized == next.index {
                    // go at least one full level deeper
                    self.wipe_cache(next.index, cur.index);
                    if next.index == leaf_index {
                        // we reached the leaf, exit
                        break;
                    } else {
                        // follow the link
                        cur_ref = next_ref;
                        continue;
                    }
                } else {
                    // find intersection
                    let inter_index = {
                        // intersection index is the longest common prefix
                        let mut i = leaf_index_normalized;
                        let mut j = next.index;
                        while i != j {
                            i >>= 1;
                            j >>= 1;
                        }
                        i
                    };

                    self.wipe_cache(inter_index, cur.index);

                    // add a split node at intersection and insert the leaf
                    let leaf_ref = self.insert_node(leaf_index, tree_depth, None, None);
                    let (lhs, rhs) = select(
                        leaf_index_normalized > next.index,
                        Some(next_ref),
                        Some(leaf_ref),
                    );
                    let inter_ref =
                        self.insert_node(inter_index, Self::depth(inter_index), lhs, rhs);
                    self.add_child_node(cur_ref, dir, inter_ref);
                    break;
                }
            } else {
                // insert the leaf node and update cur
                let leaf_ref = self.insert_node(leaf_index, tree_depth, None, None);
                self.add_child_node(cur_ref, dir, leaf_ref);
                break;
            }
        }
    }

    fn insert_node(
        &mut self,
        index: NodeIndex,
        depth: Depth,
        left: Option<NodeRef>,
        right: Option<NodeRef>,
    ) -> NodeRef {
        self.nodes.push(Node {
            index,
            depth,
            left,
            right,
        });
        self.nodes.len() - 1
    }

    fn add_child_node(&mut self, node_ref: NodeRef, dir: bool, child: NodeRef) {
        let node = &mut self.nodes[node_ref];
        if dir {
            node.right = Some(child);
        } else {
            node.left = Some(child);
        }
    }

    fn get_hash_line(&self, child_ref: NodeRef, parent: &Node) -> (Hash, Vec<(NodeIndex, Hash)>) {
        let child = &self.nodes[child_ref];

        let acc = self.get_hash(child_ref);
        let mut cur_hash = acc.0;
        let mut updates = acc.1;

        let mut cur_depth = child.depth - 1;
        let mut cur_i = child.index;

        while cur_depth > parent.depth {
            unsafe {
                HC += 1;
            }
            let swap = (cur_i & 1) == 0;
            let (lhs, rhs) = select(swap, cur_hash, self.prehashed[cur_depth + 1].clone());
            cur_hash = self
                .hasher
                .compress(&lhs, &rhs, self.tree_depth - cur_depth - 1);
            cur_depth -= 1;
            cur_i >>= 1;
            //self.cache.insert(cur_i, cur_hash.clone());
            updates.push((cur_i, cur_hash.clone()));
        }
        (cur_hash, updates)
    }

    fn get_child_hash(
        &self,
        child_ref: Option<NodeRef>,
        parent: &Node,
        dir: usize,
    ) -> (Hash, Vec<(NodeIndex, Hash)>) {
        let neighbour_index = parent.index * 2 + dir as NodeIndex;
        match self.cache.get(&neighbour_index) {
            Some(cached) => (
                cached.clone(),
                Vec::with_capacity((self.tree_depth + 1) * 2),
            ),
            None => match child_ref {
                Some(child_ref) => self.get_hash_line(child_ref, parent),
                None => (
                    self.prehashed[parent.depth + 1].clone(),
                    Vec::with_capacity((self.tree_depth + 1) * 2),
                ),
            },
        }
    }

    fn get_hash(&self, node_ref: NodeRef) -> (Hash, Vec<(NodeIndex, Hash)>) {
        let node = &self.nodes[node_ref].clone();
        let mut acc = {
            if node.depth == self.tree_depth {
                // leaf node: return item hash
                let item_index: ItemIndex = (node.index - (1 << self.tree_depth)) as ItemIndex;
                unsafe {
                    HN += 1;
                }
                let item_hash = self.hasher.hash_bits(self.items[&item_index].get_bits_le());
                (item_hash, vec![])
            } else {
                let (hl, hr) = rayon::join(
                    || self.get_child_hash(node.left, node, 0),
                    || self.get_child_hash(node.right, node, 1),
                );

                // level is used by hasher for personalization
                let level = self.tree_depth - node.depth - 1;
                let hash = self.hasher.compress(&hl.0, &hr.0, level);

                let mut updates = hl.1;
                updates.extend(hr.1);
                (hash, updates)
            }
        };
        acc.1.push((node.index, acc.0.clone()));
        //self.cache.insert(node.index, hash.clone());
        acc
    }

    pub fn root_hash(&mut self) -> Hash {
        let acc = self.get_hash(0);
        for v in acc.1 {
            self.cache.insert(v.0, v.1);
        }
        acc.0
    }

    pub fn reset_stats() {
        unsafe {
            HN = 0;
            HC = 0;
        }
    }

    pub fn print_stats() {
        // unsafe {
        //            debug!("leaf hashes: {}", HN);
        //            debug!("tree hashes: {}", HC);
        // }
    }

    pub fn make_a_future(&self) {

        //        let pool = CpuPool::new_num_cpus();
        //
        //        let r = thread::scope(|scope| {
        //            scope.spawn(move |_| {
        //                debug!("Hello! {:?}", self.root_hash());
        //                std::thread::sleep(Duration::from_millis(1400));
        //                debug!("done");
        //                3 + 5
        //            });
        //        }).unwrap();
        //        debug!("r {:?}", r);

        //        debug!("testing cpu");
        //        crossbeam_utils::thread::scope(|scope| {
        //            scope.spawn(move || {
        //                debug!("begin");
        //            })
        //        });

        //        Box::new(self.pool.spawn(future::lazy(move || {
        //            debug!("begin");
        //            let r = self.root_hash();
        //            debug!("end: {:?}", r);
        //            future::ok::<(), ()>(())
        //        })))/*.then(|result| {
        //            debug!("result {:?}", result);
        //            future::ok::<(), ()>(())
        //        }));*/
        //f.wait();
    }
}

// testing stats
// TODO: remove this for production
static mut HN: usize = 0;
static mut HC: usize = 0;

#[cfg(test)]
mod tests {
    use super::*;

    use log::debug;

    #[derive(Debug)]
    struct TestHasher {}

    #[derive(Debug)]
    struct TestLeaf(u64);

    impl Default for TestLeaf {
        fn default() -> Self {
            TestLeaf(0)
        }
    }

    impl GetBits for TestLeaf {
        fn get_bits_le(&self) -> Vec<bool> {
            let mut acc = Vec::new();
            let mut i = self.0 + 1;
            for _ in 0..16 {
                acc.push(i & 1 == 1);
                i >>= 1;
            }
            acc
        }
    }

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

        fn compress(&self, lhs: &u64, rhs: &u64, i: usize) -> u64 {
            (11 * lhs + 17 * rhs + 1 + i as u64) % 1_234_567_891
            //debug!("compress {} {}, {} => {}", lhs, rhs, i, r);
        }
    }

    type TestSMT = SparseMerkleTree<TestLeaf, u64, TestHasher>;

    use rand::{thread_rng, Rand};

    #[test]
    fn test_batching_tree_insert1() {
        let rng = &mut thread_rng();
        //        tree.insert(0, TestLeaf(0));
        //        tree.insert(3, TestLeaf(2));
        //        tree.insert(1, TestLeaf(1));
        //        tree.insert(3, TestLeaf(2));
        //        tree.insert(5, TestLeaf(2));
        //        tree.insert(7, TestLeaf(2));
        //
        //        for _ in 0..1000 {
        //            let insert_into = usize::rand(rng) % capacity;
        //            tree.insert(insert_into, TestLeaf(u64::rand(rng)));
        //            tree.root_hash();
        //        }
        //        tree.insert(usize::rand(rng) % capacity, TestLeaf(2));
        //        //debug!("{:?}\n", tree);

        let mut n = 1000;
        for _i in 0..3 {
            let mut tree = TestSMT::new(24);
            let capacity = tree.capacity();
            unsafe {
                HN = 0;
                HC = 0;
            }
            for _j in 0..n {
                let insert_into = usize::rand(rng) % capacity;
                tree.insert(insert_into, TestLeaf(2));
            }
            tree.root_hash();
            unsafe {
                debug!("{}: HN = {}, HC = {}\n", n, HN, HC);
            }
            n *= 10;
        }
    }

    #[test]
    fn test_batching_tree_insert_comparative() {
        let mut tree = TestSMT::new(3);
        tree.insert(0, TestLeaf(1));
        assert_eq!(tree.root_hash(), 697_516_875);
        tree.insert(0, TestLeaf(2));
        assert_eq!(tree.root_hash(), 741_131_083);
        tree.insert(3, TestLeaf(2));
        assert_eq!(tree.root_hash(), 793_215_819);
    }

    #[test]
    fn test_cpu_pool() {
        let tree = TestSMT::new(3);

        tree.make_a_future();

        //        tree.insert(0,  TestLeaf(1));
        //        debug!("{}", tree.root_hash());
        //        debug!("{:?}", tree.prehashed);
        //        debug!("{:?}", tree.nodes);
        //
        //        tree.insert(0, TestLeaf(2));
        //        debug!("{}", tree.root_hash());
        //        debug!("{:?}", tree.nodes);
    }
}
