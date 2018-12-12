use pairing::bn256::{Bn256, Fr};
use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};
use crate::models::*;

type CurveUsed = Bn256;
type FrUsed = Fr;

pub type Account = account::Account<CurveUsed>;
pub type AccountTree = SparseMerkleTree<Account, FrUsed, PedersenHasher<CurveUsed>>;
pub type Tx = tx::Tx<CurveUsed>;
pub type Block = block::Block<CurveUsed>;
pub type TransactionSignature = tx::TransactionSignature<CurveUsed>;