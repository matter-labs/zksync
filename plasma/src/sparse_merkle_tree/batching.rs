// Sparse Merkle tree with batch updates

use std::collections::HashMap;
use super::hasher::Hasher;
use super::super::primitives::GetBits;


fn select<T>(condition: bool, a: T, b: T) -> (T, T) {
    if condition { (a, b) } else { (b, a) }
}


// Lead index: 0 <= i < N
type ItemIndex = usize;

// Tree of depth 0: 1 item (which is root), level 0 only
// Tree of depth 1: 2 items, levels 0 and 1
// Tree of depth N: 2 ^ N items, 0 <= level < depth
type Depth = usize;

// Nodes enumarated starting with index(root) = 1
type NodeIndex = usize;

// Index of the node in the vector; slightly inefficient, won't be needed when rust gets non-lexical timelines
type NodeRef = usize;

#[derive(Debug, Clone)]
struct Node {
    depth: Depth,
    index: NodeIndex,
    lhs: Option<NodeRef>,
    rhs: Option<NodeRef>,
}

#[derive(Debug, Clone)]
pub struct SparseMerkleTree<T: GetBits + Default, Hash: Clone, H: Hasher<Hash>>
{
    tree_depth: Depth,
    prehashed: Vec<Hash>,
    items: HashMap<ItemIndex, T>,
    hasher: H,

    // intermediate nodes
    root: NodeRef,
    nodes_v: Vec<Node>,
}

impl<T, Hash, H> SparseMerkleTree< T, Hash, H>
    where T: GetBits + Default,
          Hash: Clone,
          H: Hasher<Hash> + Default,
{

    pub fn new(tree_depth: Depth) -> Self {
        assert!(tree_depth > 1);
        let hasher = H::default();
        let items = HashMap::new();
        let mut nodes_v = Vec::new();
        nodes_v.push(Node{
            index: 1,
            depth: 0,
            lhs: None,
            rhs: None,
        });

        let mut prehashed = Vec::with_capacity(tree_depth);
        let mut cur = hasher.hash_bits(T::default().get_bits_le());
        prehashed.push(cur.clone());
        for i in 0..tree_depth {
            cur = hasher.compress(&cur, &cur, i);
            prehashed.push(cur.clone());
        }
        prehashed.reverse();

        Self{tree_depth, prehashed, items, hasher, nodes_v, root: 0}
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
    fn nodes_capacity(&self) -> usize {
        (1 << (self.tree_depth + 1)) - 1
    }

    pub fn insert(&mut self, item_index: ItemIndex, item: T) {
        assert!(item_index < self.capacity());
        let tree_depth = self.tree_depth;
        let leaf_index = (1 << tree_depth) + item_index;
        //println!("\ninsert item_index = {}, leaf_index = {:?}", item_index, leaf_index);

        let leaf_ref = {
            self.insert_node(leaf_index, tree_depth, None, None)
        };

        if let None = self.items.insert(item_index, item) {
            // inserting an item at a new index

            // traverse the tree
            let mut cur_ref = self.root;
            loop {
                let cur = { self.nodes_v[cur_ref].clone() };

                //println!("cur_i = {:?}", cur_i);
                //println!("cur_node = {:?}", cur_node);

                let dir = (leaf_index & (1 << (tree_depth - cur.depth - 1))) > 0;
                //println!("dir = {:?}", dir);
                let mut link = if dir { cur.rhs } else { cur.lhs };
                if let Some(next_ref) = link {
                    let next = { self.nodes_v[next_ref].clone() };
                    let leaf_index_normalized = leaf_index >> (tree_depth - next.depth);
                    //println!("next = {}, leaf_index_normalized = {:?}, next_depth = {:?}", next, leaf_index_normalized, next_depth);

                    if leaf_index_normalized == next.index {
                        // follow the link
                        cur_ref = next_ref;
                        continue;
                    } else {
                        // split at intersection
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
                        //println!("intersection = {:?}", intersection_i);

                        let (lhs, rhs) = select(leaf_index_normalized > next.index, Some(next_ref), Some(leaf_ref));
                        let inter_ref = self.insert_node(inter_index, Self::depth(inter_index), lhs, rhs);
                        //println!("node[{}] = {:?}", intersection_i, inter_node);
                        self.add_child(cur_ref, dir, inter_ref);
                        break;
                    }
                } else {
                    // insert the leaf node and update cur
                    self.add_child(cur_ref, dir, leaf_ref);
                    break;
                }
            }
        }

    }

    fn add_child(&mut self, r: NodeRef, dir: bool, child: NodeRef) {
        let node = &mut self.nodes_v[r];
        if dir {
            node.rhs = Some(child);
        } else {
            node.lhs = Some(child);
        }
    }

    fn insert_node(&mut self, index: NodeIndex, depth: Depth, lhs: Option<NodeRef>, rhs: Option<NodeRef>) -> NodeRef {
        self.nodes_v.push(Node{index, depth, lhs, rhs});
        self.nodes_v.len() - 1
    }

    // optimization to reduce num of mem allocs
    pub fn prepare_inserts(&mut self, n: usize) {
        self.items.reserve(n);
        self.nodes_v.reserve(2 * n);
    }

    fn hash_line(&self, from: Option<NodeRef>, to: &Node, dir: bool) -> Hash {
        //println!("hash_line {:?} {} {}", from, to, dir);

        match from {
            None => self.prehashed[to.depth + 1].clone(),
            Some(from_ref) => {
                let from = &self.nodes_v[from_ref];
                let mut cur_hash = self.get_hash(from);
                let mut cur_depth = from.depth - 1;
                while cur_depth > to.depth {
                    //println!("cur_depth = {}", cur_depth);
                    let (lhs, rhs) = select(!dir, cur_hash, self.prehashed[cur_depth + 1].clone());
                    cur_hash = self.hasher.compress(&lhs, &rhs, self.tree_depth - cur_depth - 1);
                    cur_depth -= 1;
                }
                cur_hash
            }
        }
    }

    fn get_hash(&self, node: &Node) -> Hash {
        //println!("get_hash {}", index);

        if node.depth == self.tree_depth {
            // leaf node: return item hash
            let item_index = node.index - (1 << self.tree_depth);
            //println!("item_index = {}", item_index);
            return self.hasher.hash_bits(self.items[&item_index].get_bits_le())
        }

        let lhs = self.hash_line(node.lhs, node, false);
        let rhs = self.hash_line(node.rhs, node, true);
        self.hasher.compress(&lhs, &rhs, self.tree_depth - node.depth - 1)
    }

    pub fn root_hash(&self) -> Hash {
        self.get_hash(&self.nodes_v[0])
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestHasher {}

    #[derive(Debug)]
    struct TestLeaf(u64);

    impl Default for TestLeaf {
        fn default() -> Self { TestLeaf(0) }
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
            let r = (11 * lhs + 17 * rhs + 1 + i as u64) % 1234567891;
            //println!("compress {} {}, {} => {}", lhs, rhs, i, r);
            r
        }

    }

    type TestSMT = SparseMerkleTree<TestLeaf, u64, TestHasher>;

    use rand::{Rand, thread_rng};

    #[test]
    fn test_batching_tree_insert1() {
        let rng = &mut thread_rng();
        let mut tree = TestSMT::new(8);
        let capacity = tree.capacity();
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
//        //println!("{:?}\n", tree);

        for j in 0..700 {
            //b.iter(|| {
            let insert_into = usize::rand(rng) % capacity;
            //println!("insert_into {}", insert_into);
            //for i in 0..leafs.len() {
            let i = 0;
            tree.insert(insert_into, TestLeaf(2));

            //println!("{:?}", tree);
            //println!("{:?}", tree.nodes);
            tree.root_hash();
            //println!("{:?}", tree.root_hash());
            //println!("{:?}", tree.root_hash());

            //}
            //tree.root_hash();
            //});
        }

        //println!("{:?}\n", tree.root_hash());
    }

    #[test]
    fn test_batching_tree_insert_comparative() {
        let mut tree = TestSMT::new(3);

        tree.insert(0,  TestLeaf(1));
        //println!("{:?}", tree);
        assert_eq!(tree.root_hash(), 697516875);

        tree.insert(3, TestLeaf(2));
        //println!("{:?}", tree);
        assert_eq!(tree.root_hash(), 749601611);
    }

}
