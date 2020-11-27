use crate::block::Block;
use ethabi::Token;
use serde::{Deserialize, Serialize};
use zksync_basic_types::{BlockNumber, H256, U256};
use zksync_crypto::proof::EncodedAggregatedProof;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksCommitOperation {
    pub last_committed_block: Block,
    pub blocks: Vec<Block>,
}

pub fn stored_block_info(block: &Block) -> Token {
    Token::Tuple(vec![
        Token::Uint(U256::from(block.block_number)),
        Token::Uint(U256::from(block.number_of_processed_prior_ops())),
        Token::FixedBytes(
            block
                .get_onchain_operations_block_info()
                .1
                .as_bytes()
                .to_vec(),
        ),
        Token::Uint(U256::from(block.timestamp)),
        Token::FixedBytes(block.get_eth_encoded_root().as_bytes().to_vec()),
        Token::FixedBytes(block.block_commitment.as_bytes().to_vec()),
    ])
}

impl BlocksCommitOperation {
    pub fn get_eth_tx_args(&self) -> Vec<Token> {
        let stored_block_info = stored_block_info(&self.last_committed_block);
        let blocks_to_commit = self
            .blocks
            .iter()
            .map(|block| {
                let onchain_ops = block
                    .get_onchain_operations_block_info()
                    .0
                    .into_iter()
                    .map(|op| {
                        Token::Tuple(vec![
                            Token::Uint(U256::from(op.public_data_offset)),
                            Token::Bytes(op.eth_witness),
                        ])
                    })
                    .collect::<Vec<_>>();
                Token::Tuple(vec![
                    Token::Uint(U256::from(block.block_number)),
                    Token::Uint(U256::from(block.fee_account)),
                    Token::FixedBytes(block.get_eth_encoded_root().as_bytes().to_vec()),
                    Token::Bytes(block.get_eth_public_data()),
                    Token::Uint(U256::from(block.timestamp)),
                    Token::Array(onchain_ops),
                ])
            })
            .collect();

        vec![stored_block_info, Token::Array(blocks_to_commit)]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksProofOperation {
    pub blocks: Vec<Block>,
    pub proof: EncodedAggregatedProof,
    pub block_idxs_in_proof: Vec<usize>,
}

impl BlocksProofOperation {
    pub fn get_eth_tx_args(&self) -> Vec<Token> {
        let blocks_arg = Token::Array(self.blocks.iter().map(|b| stored_block_info(b)).collect());

        let committed_idxs = Token::Array(
            self.block_idxs_in_proof
                .iter()
                .map(|idx| Token::Uint(U256::from(*idx)))
                .collect(),
        );

        let proof = self.proof.get_eth_tx_args();

        vec![blocks_arg, committed_idxs, proof]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExecuteOperationArg {
    pub block: Block,
}

impl BlockExecuteOperationArg {
    fn get_eth_tx_args(&self) -> Token {
        let stored_block = stored_block_info(&self.block);

        let processable_ops_pubdata = Token::Array(
            self.block
                .processable_ops_pubdata()
                .into_iter()
                .map(|pubdata| Token::Bytes(pubdata))
                .collect(),
        );

        Token::Tuple(vec![stored_block, processable_ops_pubdata])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksExecuteOperation {
    pub blocks: Vec<BlockExecuteOperationArg>,
}

impl BlocksExecuteOperation {
    pub fn get_eth_tx_args(&self) -> Vec<Token> {
        vec![Token::Array(
            self.blocks
                .iter()
                .map(|arg| arg.get_eth_tx_args())
                .collect(),
        )]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AggregatedActionType {
    CommitBlocks,
    CreateProofBlocks,
    PublishProofBlocksOnchain,
    ExecuteBlocks,
}
impl std::string::ToString for AggregatedActionType {
    fn to_string(&self) -> String {
        match self {
            AggregatedActionType::CommitBlocks => "CommitBlocks".to_owned(),
            AggregatedActionType::CreateProofBlocks => "CreateProofBlocks".to_owned(),
            AggregatedActionType::PublishProofBlocksOnchain => {
                "PublishProofBlocksOnchain".to_owned()
            }
            AggregatedActionType::ExecuteBlocks => "ExecuteBlocks".to_owned(),
        }
    }
}

impl std::str::FromStr for AggregatedActionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "CommitBlocks" => Ok(Self::CommitBlocks),
            "CreateProofBlocks" => Ok(Self::CreateProofBlocks),
            "PublishProofBlocksOnchain" => Ok(Self::PublishProofBlocksOnchain),
            "ExecuteBlocks" => Ok(Self::ExecuteBlocks),
            _ => Err("Incorrect aggregated action type".to_owned()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregatedOperation {
    CommitBlocks(BlocksCommitOperation),
    CreateProofBlocks(Vec<BlockNumber>),
    PublishProofBlocksOnchain(BlocksProofOperation),
    ExecuteBlocks(BlocksExecuteOperation),
}

impl AggregatedOperation {
    pub fn get_action_type(&self) -> AggregatedActionType {
        match self {
            AggregatedOperation::CommitBlocks(..) => AggregatedActionType::CommitBlocks,
            AggregatedOperation::CreateProofBlocks(..) => AggregatedActionType::CreateProofBlocks,
            AggregatedOperation::PublishProofBlocksOnchain(..) => {
                AggregatedActionType::PublishProofBlocksOnchain
            }
            AggregatedOperation::ExecuteBlocks(..) => AggregatedActionType::ExecuteBlocks,
        }
    }

    pub fn get_block_range(&self) -> (BlockNumber, BlockNumber) {
        match self {
            AggregatedOperation::CommitBlocks(BlocksCommitOperation { blocks, .. }) => (
                blocks.first().map(|b| b.block_number).unwrap_or_default(),
                blocks.last().map(|b| b.block_number).unwrap_or_default(),
            ),
            AggregatedOperation::CreateProofBlocks(blocks) => (
                blocks.first().cloned().unwrap_or_default(),
                blocks.last().cloned().unwrap_or_default(),
            ),
            AggregatedOperation::PublishProofBlocksOnchain(BlocksProofOperation {
                blocks, ..
            }) => (
                blocks.first().map(|c| c.block_number).unwrap_or_default(),
                blocks.last().map(|c| c.block_number).unwrap_or_default(),
            ),
            AggregatedOperation::ExecuteBlocks(BlocksExecuteOperation { blocks }) => (
                blocks
                    .first()
                    .map(|b| b.block.block_number)
                    .unwrap_or_default(),
                blocks
                    .last()
                    .map(|b| b.block.block_number)
                    .unwrap_or_default(),
            ),
        }
    }
}
