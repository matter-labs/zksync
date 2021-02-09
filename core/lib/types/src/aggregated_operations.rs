use crate::block::Block;
use ethabi::Token;
use serde::{Deserialize, Serialize};
use zksync_basic_types::{BlockNumber, U256};
use zksync_crypto::proof::EncodedAggregatedProof;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksCommitOperation {
    pub last_committed_block: Block,
    pub blocks: Vec<Block>,
}

pub fn stored_block_info(block: &Block) -> Token {
    Token::Tuple(vec![
        Token::Uint(U256::from(*block.block_number)),
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
                            Token::Bytes(op.eth_witness),
                            Token::Uint(U256::from(op.public_data_offset)),
                        ])
                    })
                    .collect::<Vec<_>>();
                Token::Tuple(vec![
                    Token::FixedBytes(block.get_eth_encoded_root().as_bytes().to_vec()),
                    Token::Bytes(block.get_eth_public_data()),
                    Token::Uint(U256::from(block.timestamp)),
                    Token::Array(onchain_ops),
                    Token::Uint(U256::from(*block.block_number)),
                    Token::Uint(U256::from(*block.fee_account)),
                ])
            })
            .collect();

        vec![stored_block_info, Token::Array(blocks_to_commit)]
    }

    pub fn block_range(&self) -> (BlockNumber, BlockNumber) {
        let BlocksCommitOperation { blocks, .. } = self;
        (
            blocks.first().map(|b| b.block_number).unwrap_or_default(),
            blocks.last().map(|b| b.block_number).unwrap_or_default(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksCreateProofOperation {
    pub blocks: Vec<Block>,
    pub proofs_to_pad: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksProofOperation {
    pub blocks: Vec<Block>,
    pub proof: EncodedAggregatedProof,
}

impl BlocksProofOperation {
    pub fn get_eth_tx_args(&self) -> Vec<Token> {
        let blocks_arg = Token::Array(self.blocks.iter().map(|b| stored_block_info(b)).collect());

        let proof = self.proof.get_eth_tx_args();

        vec![blocks_arg, proof]
    }

    pub fn block_range(&self) -> (BlockNumber, BlockNumber) {
        let BlocksProofOperation { blocks, .. } = self;
        (
            blocks.first().map(|c| c.block_number).unwrap_or_default(),
            blocks.last().map(|c| c.block_number).unwrap_or_default(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksExecuteOperation {
    pub blocks: Vec<Block>,
}

impl BlocksExecuteOperation {
    fn get_eth_tx_args_for_block(block: &Block) -> Token {
        let stored_block = stored_block_info(&block);

        let processable_ops_pubdata = Token::Array(
            block
                .processable_ops_pubdata()
                .into_iter()
                .map(Token::Bytes)
                .collect(),
        );

        Token::Tuple(vec![stored_block, processable_ops_pubdata])
    }

    pub fn get_eth_tx_args(&self) -> Vec<Token> {
        vec![Token::Array(
            self.blocks
                .iter()
                .map(BlocksExecuteOperation::get_eth_tx_args_for_block)
                .collect(),
        )]
    }

    pub fn block_range(&self) -> (BlockNumber, BlockNumber) {
        let BlocksExecuteOperation { blocks } = self;
        (
            blocks.first().map(|b| b.block_number).unwrap_or_default(),
            blocks.last().map(|b| b.block_number).unwrap_or_default(),
        )
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
    CreateProofBlocks(BlocksCreateProofOperation),
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
            AggregatedOperation::CommitBlocks(op) => op.block_range(),
            AggregatedOperation::CreateProofBlocks(BlocksCreateProofOperation {
                blocks, ..
            }) => (
                blocks.first().map(|c| c.block_number).unwrap_or_default(),
                blocks.last().map(|c| c.block_number).unwrap_or_default(),
            ),
            AggregatedOperation::PublishProofBlocksOnchain(op) => op.block_range(),
            AggregatedOperation::ExecuteBlocks(op) => op.block_range(),
        }
    }

    pub fn is_commit(&self) -> bool {
        matches!(self.get_action_type(), AggregatedActionType::CommitBlocks)
    }

    pub fn is_execute(&self) -> bool {
        matches!(self.get_action_type(), AggregatedActionType::ExecuteBlocks)
    }

    pub fn is_create_proof(&self) -> bool {
        matches!(
            self.get_action_type(),
            AggregatedActionType::CreateProofBlocks
        )
    }

    pub fn is_publish_proofs(&self) -> bool {
        matches!(
            self.get_action_type(),
            AggregatedActionType::PublishProofBlocksOnchain
        )
    }
}

impl From<BlocksCommitOperation> for AggregatedOperation {
    fn from(other: BlocksCommitOperation) -> Self {
        Self::CommitBlocks(other)
    }
}
impl From<BlocksCreateProofOperation> for AggregatedOperation {
    fn from(other: BlocksCreateProofOperation) -> Self {
        Self::CreateProofBlocks(other)
    }
}

impl From<BlocksProofOperation> for AggregatedOperation {
    fn from(other: BlocksProofOperation) -> Self {
        Self::PublishProofBlocksOnchain(other)
    }
}

impl From<BlocksExecuteOperation> for AggregatedOperation {
    fn from(other: BlocksExecuteOperation) -> Self {
        Self::ExecuteBlocks(other)
    }
}
