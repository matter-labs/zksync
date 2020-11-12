use crate::block::Block;
use serde::{Deserialize, Serialize};
use zksync_basic_types::{BlockNumber, H256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksCommitOperation {
    pub last_committed_block: Block,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksProofOperation {
    pub commitments: Vec<(H256, BlockNumber)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExecuteOperationArg {
    pub block: Block,
    pub commitments: Vec<H256>,
    pub commitment_idx: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksExecuteOperation {
    pub blocks: Vec<BlockExecuteOperationArg>,
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
                commitments,
            }) => (
                commitments.first().map(|c| c.1).unwrap_or_default(),
                commitments.last().map(|c| c.1).unwrap_or_default(),
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
