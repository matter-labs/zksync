pub mod params;
pub mod account;
pub mod state;
pub mod circuit;
pub mod block;
pub mod tx;

use ff::{Field, PrimeField, PrimeFieldRepr};
use pairing::bn256;
use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};
use sapling_crypto::jubjub::{JubjubEngine, JubjubParams, edwards};
use sapling_crypto::eddsa::{Signature};

pub use self::account::Account;
pub use self::tx::{TransferTx, DepositTx, ExitTx};
pub use self::state::PlasmaState;

type Engine = bn256::Bn256;
type Fr = bn256::Fr;

pub type FieldBytes = Fr;

// TxSignature uses only native Rust types
#[derive(Clone, Serialize, Deserialize)]
pub struct TxSignature{
    pub r_compressed:    [u8; 32], // top bit is a sign
    pub s:               [u8; 32],
}

impl TxSignature{
    pub fn to_jubjub_eddsa<E: JubjubEngine>(
        &self, 
        params: &E::Params
    )
    -> Result<Signature<E>, String>
    {
        // TxSignature has S and R in compressed form serialized as BE
        let x_sign = self.r_compressed[0] & 0x80 > 0;
        let mut tmp = self.r_compressed.clone();
        tmp[0] &= 0x7f; // strip the top bit

        // read from byte array
        let y_repr = E::Fr::zero().into_repr();
        y_repr.read_be(&tmp[..]).expect("read R_y as field element");

        let s_repr = E::Fs::zero().into_repr();
        s_repr.read_be(&self.s[..]).expect("read S as field element");

        let y = E::Fr::from_repr(y_repr).expect("make y from representation");

        // here we convert it to field elements for all further uses
        let r = edwards::Point::get_for_y(y, x_sign, params);
        if r.is_none() {
            return Err("Invalid R point".to_string());
        }

        let s = E::Fs::from_repr(s_repr).expect("make s from representation");

        Ok(Signature {
            r: r.unwrap(),
            s: s
        })
    }
}

pub type AccountTree = SparseMerkleTree<Account, Fr, PedersenHasher<Engine>>;

pub type TransferBlock = block::Block<TransferTx>;
pub type DepositBlock = block::Block<DepositTx>;
pub type ExitBlock = block::Block<ExitTx>;

#[derive(Clone)]
pub enum Block {
    Transfer(TransferBlock),
    Deposit(DepositBlock),
    Exit(ExitBlock)
}