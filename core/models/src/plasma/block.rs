use crate::plasma::tx::FranklinTx;
pub use crate::plasma::tx::{DepositTx, ExitTx, TransferTx, TxSignature};
use crate::plasma::{BatchNumber, BlockNumber, Fr};
use bigdecimal::BigDecimal;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BlockData {
    Transfer {
        //#[serde(skip)]
        transactions: Vec<TransferTx>,
        total_fees: BigDecimal,
    },
    Deposit {
        //#[serde(skip)]
        transactions: Vec<DepositTx>,
        batch_number: BatchNumber,
    },
    Exit {
        //#[serde(skip)]
        transactions: Vec<ExitTx>,
        batch_number: BatchNumber,
    },
}

// #[derive(Clone, Serialize, Deserialize)]
// pub enum BlockType { Transfer, Deposit, Exit }

// impl BlockData {
//     fn block_type(&self) -> BlockType {
//         match self {
//             BlockData::Transfer{transactions: _, total_fees: _} => BlockType::Transfer,
//             BlockData::Deposit{transactions: _, batch_number: _} => BlockType::Deposit,
//             BlockData::Exit{transactions: _, batch_number: _} => BlockType::Exit,
//         }
//     }
// }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub block_number: BlockNumber,
    pub new_root_hash: Fr,
    pub block_data: Vec<FranklinTx>,
}

impl Block {
    pub fn get_eth_public_data(&self) -> Vec<u8> {
        // TODO: (Drogan) implement using new public data spec.
        Vec::new()
    }
}
