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

type NodeIndex = usize;

#[derive(Debug, Clone)]
struct Node {
    lhs: Option<NodeIndex>,
    rhs: Option<NodeIndex>,
}

#[derive(Debug, Clone)]
pub struct SparseMerkleTree<T: GetBits + Default, Hash: Clone, H: Hasher<Hash>>
{
    tree_depth: Depth,
    prehashed: Vec<Hash>,
    items: HashMap<ItemIndex, T>,
    hasher: H,
    //root: Node,

    // intermediate nodes
    nodes: HashMap<usize, Node>,
}

impl<T, Hash, H> SparseMerkleTree<T, Hash, H>
    where T: GetBits + Default,
          Hash: Clone,
          H: Hasher<Hash> + Default,
{

    pub fn new(tree_depth: Depth) -> Self {
        assert!(tree_depth > 1);
        let hasher = H::default();
        let items = HashMap::new();
        let mut nodes = HashMap::new();
        nodes.insert(1, Node{
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

        Self{tree_depth, prehashed, items, hasher, nodes}
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

    fn update_node(&mut self, cur_node: &Node, cur_i: NodeIndex, dir: bool, child: NodeIndex) {
        let mut updated_cur = cur_node.clone();
        {
            let link = if dir { &mut updated_cur.rhs } else { &mut updated_cur.lhs };
            *link = Some(child);
        }
        //println!("node[{}] = {:?}", cur_i, updated_cur.clone());
        self.nodes.insert(cur_i, updated_cur);
    }

    pub fn insert(&mut self, item_index: ItemIndex, item: T) {
        assert!(item_index < self.capacity());
        let leaf_index = (1 << self.tree_depth) + item_index;
        //println!("\ninsert item_index = {}, leaf_index = {:?}", item_index, leaf_index);
        if let None = self.items.insert(item_index, item) {
            // inserting an item at a new index

            // traverse the tree
            let mut cur_i = 1; // we start at root
            loop {
                let cur_node = self.nodes.get(&cur_i).unwrap().clone(); // must be present
                //println!("cur_i = {:?}", cur_i);
                //println!("cur_node = {:?}", cur_node);

                let cur_depth = Self::depth(cur_i);
                let dir = (leaf_index & (1 << (self.tree_depth - cur_depth - 1))) > 0;
                //println!("dir = {:?}", dir);
                let link = if dir { cur_node.rhs } else { cur_node.lhs };

                if let Some(next) = link {
                    let next_depth = Self::depth(next);
                    let leaf_index_normalized = leaf_index >> (self.tree_depth - next_depth);
                    //println!("next = {}, leaf_index_normalized = {:?}, next_depth = {:?}", next, leaf_index_normalized, next_depth);

                    if leaf_index_normalized == next {
                        // follow the link
                        cur_i = next;
                        continue;

                    } else {
                        // split at intersection
                        let intersection_i = {
                            let mut i = leaf_index_normalized;
                            while (i & 1) != (next & 1) { i >>= 1; }
                            i
                        };
                        //println!("intersection = {:?}", intersection_i);

                        let (lhs, rhs) = select(leaf_index_normalized > next, Some(next), Some(leaf_index));
                        let intersection_node = Node{lhs, rhs};
                        //println!("node[{}] = {:?}", intersection_i, intersection_node);
                        self.nodes.insert(intersection_i, intersection_node);

                        self.update_node(&cur_node, cur_i, dir, intersection_i);
                        break;
                    }
                } else {
                    // insert the leaf node by updating the value of cur
                    self.update_node(&cur_node, cur_i, dir, leaf_index);
                    break;
                }

            }
        }

    }

    fn hash_line(&self, from: Option<NodeIndex>, to: NodeIndex, dir: bool) -> Hash {
        println!("hash_line {:?} {} {}", from, to, dir);

        let to_depth = Self::depth(to);
        match from {
            None => self.prehashed[to_depth + 1].clone(),
            Some(from) => {
                let mut cur_hash = self.get_hash(from);
                let mut cur_depth = Self::depth(from) - 1;
                while cur_depth > to_depth {
                    println!("cur_depth = {}", cur_depth);
                    let (lhs, rhs) = select(!dir, cur_hash.clone(), self.prehashed[cur_depth + 1].clone());
                    cur_hash = self.hasher.compress(&lhs, &rhs, self.tree_depth - cur_depth - 1);
                    cur_depth -= 1;
                }
                cur_hash
            }
        }
    }

    fn get_hash(&self, index: NodeIndex) -> Hash {
        //println!("get_hash {}", index);

        let cur_depth = Self::depth(index);
        if cur_depth == self.tree_depth {
            // leaf node: return item hash
            let item_index = index - (1 << self.tree_depth);
            //println!("item_index = {}", item_index);
            return self.hasher.hash_bits(self.items[&item_index].get_bits_le())
        }

        let cur_node = self.nodes.get(&index).unwrap().clone();
        //println!("cur_node {:?}", cur_node);

        let lhs = self.hash_line(cur_node.lhs, index, false);
        let rhs = self.hash_line(cur_node.rhs, index, true);
        self.hasher.compress(&lhs, &rhs, self.tree_depth - cur_depth - 1)
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
            let r = 11 * lhs + 17 * rhs + 1 + i as u64;
            //println!("compress {} {}, {} => {}", lhs, rhs, i, r);
            r
        }

    }

    type TestSMT = SparseMerkleTree<TestLeaf, u64, TestHasher>;


    #[test]
    fn test_batching_tree_insert() {
        let mut tree = TestSMT::new(3);
        tree.insert(0, TestLeaf(1));
        tree.insert(3, TestLeaf(2));
        tree.insert(1, TestLeaf(2));
        tree.insert(3, TestLeaf(2));
        tree.insert(5, TestLeaf(2));
        tree.insert(2, TestLeaf(2));
        //println!("{:?}\n", tree);
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
