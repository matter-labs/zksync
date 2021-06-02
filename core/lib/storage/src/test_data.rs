//! Utilities used to generate test data for tests that involve database interaction.

// Built-in uses
use std::ops::Deref;

// External imports
use num::{BigUint, Zero};
use once_cell::sync::Lazy;
use parity_crypto::publickey::{Generator, Random};
// Workspace imports
use zksync_crypto::params::MIN_NFT_TOKEN_ID;
use zksync_crypto::proof::{AggregatedProof, PrecomputedSampleProofs, SingleProof};
use zksync_crypto::{ff::PrimeField, rand::Rng, Fr};
use zksync_prover_utils::fs_utils::load_precomputed_proofs;
use zksync_types::{
    account::Account,
    aggregated_operations::{
        AggregatedActionType, AggregatedOperation, BlocksCommitOperation,
        BlocksCreateProofOperation, BlocksExecuteOperation, BlocksProofOperation,
    },
    tx::{EthSignData, PackedEthSignature, TxEthSignature},
    Action, Address, Operation, H256, NFT,
    {
        block::{Block, ExecutedOperations},
        AccountId, AccountUpdate, BlockNumber, Nonce, PubKeyHash, TokenId,
    },
};
// Local imports

static SAMPLE_PROOF: Lazy<PrecomputedSampleProofs> =
    Lazy::new(|| load_precomputed_proofs().unwrap());

pub fn get_sample_single_proof() -> SingleProof {
    SAMPLE_PROOF.single_proofs[0].0.clone()
}

pub fn get_sample_aggregated_proof() -> AggregatedProof {
    SAMPLE_PROOF.aggregated_proof.clone()
}

/// Block size used for tests
pub const BLOCK_SIZE_CHUNKS: usize = 100;

/// Generates a random account with a set of changes.
pub fn gen_acc_random_updates<R: Rng>(
    rng: &mut R,
) -> impl Iterator<Item = (AccountId, AccountUpdate)> {
    let id: AccountId = AccountId(rng.gen());
    let balance = u128::from(rng.gen::<u64>());
    let nonce = Nonce(rng.gen());
    let pub_key_hash = PubKeyHash { data: rng.gen() };
    let address: Address = rng.gen::<[u8; 20]>().into();

    let mut a = Account::default_with_address(&address);
    let old_nonce = nonce;
    a.nonce = old_nonce + 3;
    a.pub_key_hash = pub_key_hash;

    let old_balance = a.get_balance(TokenId(0));
    a.set_balance(TokenId(0), BigUint::from(balance));
    let new_balance = a.get_balance(TokenId(0));

    vec![
        (
            id,
            AccountUpdate::Create {
                nonce: old_nonce,
                address: a.address,
            },
        ),
        (
            id,
            AccountUpdate::ChangePubKeyHash {
                old_nonce,
                old_pub_key_hash: PubKeyHash::default(),
                new_nonce: old_nonce + 1,
                new_pub_key_hash: a.pub_key_hash,
            },
        ),
        (
            id,
            AccountUpdate::UpdateBalance {
                old_nonce: old_nonce + 1,
                new_nonce: old_nonce + 2,
                balance_update: (TokenId(0), old_balance, new_balance),
            },
        ),
    ]
    .into_iter()
}

/// Generates one nft.
pub fn generate_nft(
    account_id: AccountId,
    account: &Account,
    number: u32,
) -> Vec<(AccountId, AccountUpdate)> {
    let nft = NFT::new(
        TokenId(MIN_NFT_TOKEN_ID + number),
        number,
        account_id,
        account.address,
        Address::random(),
        None,
        H256::random(),
    );
    vec![
        (account_id, AccountUpdate::MintNFT { token: nft }),
        (
            account_id,
            AccountUpdate::UpdateBalance {
                old_nonce: account.nonce,
                new_nonce: account.nonce,
                balance_update: (
                    TokenId(MIN_NFT_TOKEN_ID + number),
                    BigUint::zero(),
                    BigUint::from(1u32),
                ),
            },
        ),
    ]
}
/// Generates EthSignData for testing (not a valid signature)
pub fn gen_eth_sign_data(message: String) -> EthSignData {
    let keypair = Random.generate();
    let private_key = keypair.secret();

    let signature = PackedEthSignature::sign(private_key.deref(), &message.as_bytes()).unwrap();

    EthSignData {
        signature: TxEthSignature::EthereumSignature(signature),
        message: message.into_bytes(),
    }
}

/// Creates a dummy new root hash for the block based on its number.
pub fn dummy_root_hash_for_block(block_number: BlockNumber) -> Fr {
    Fr::from_str(&block_number.to_string()).unwrap()
}

/// Creates a dummy ethereum operation hash based on its number.
pub fn dummy_ethereum_tx_hash(ethereum_op_id: i64) -> H256 {
    H256::from_low_u64_ne(ethereum_op_id as u64)
}

/// Generates dummy operation with the unique `new_root_hash` in the block.
pub fn gen_unique_operation(
    block_number: BlockNumber,
    action: Action,
    block_chunks_size: usize,
) -> Operation {
    gen_unique_operation_with_txs(block_number, action, block_chunks_size, vec![])
}

/// Generates dummy aggregated operation with the unique `new_root_hash` in the block.
pub fn gen_unique_aggregated_operation(
    block_number: BlockNumber,
    action: AggregatedActionType,
    block_chunks_size: usize,
) -> AggregatedOperation {
    gen_unique_aggregated_operation_with_txs(block_number, action, block_chunks_size, vec![])
}

pub fn gen_sample_block(
    block_number: BlockNumber,
    block_chunks_size: usize,
    txs: Vec<ExecutedOperations>,
) -> Block {
    Block {
        block_number,
        new_root_hash: dummy_root_hash_for_block(block_number),
        fee_account: AccountId(0),
        block_transactions: txs,
        processed_priority_ops: (0, 0),
        block_chunks_size,
        commit_gas_limit: 1_000_000.into(),
        verify_gas_limit: 1_500_000.into(),
        block_commitment: H256::zero(),
        timestamp: 0,
    }
}

/// Generates dummy operation with the unique `new_root_hash` in the block and
/// given set of transactions..
pub fn gen_unique_operation_with_txs(
    block_number: BlockNumber,
    action: Action,
    block_chunks_size: usize,
    txs: Vec<ExecutedOperations>,
) -> Operation {
    Operation {
        id: None,
        action,
        block: gen_sample_block(block_number, block_chunks_size, txs),
    }
}

/// Generates dummy operation with the unique `new_root_hash` in the block and
/// given set of transactions..
pub fn gen_unique_aggregated_operation_with_txs(
    block_number: BlockNumber,
    action: AggregatedActionType,
    block_chunks_size: usize,
    txs: Vec<ExecutedOperations>,
) -> AggregatedOperation {
    let block = gen_sample_block(block_number, block_chunks_size, txs);

    match action {
        AggregatedActionType::CommitBlocks => {
            AggregatedOperation::CommitBlocks(BlocksCommitOperation {
                last_committed_block: block.clone(),
                blocks: vec![block],
            })
        }
        AggregatedActionType::CreateProofBlocks => {
            AggregatedOperation::CreateProofBlocks(BlocksCreateProofOperation {
                blocks: vec![block],
                proofs_to_pad: 0,
            })
        }
        AggregatedActionType::PublishProofBlocksOnchain => {
            AggregatedOperation::PublishProofBlocksOnchain(BlocksProofOperation {
                blocks: vec![block],
                proof: Default::default(),
            })
        }
        AggregatedActionType::ExecuteBlocks => {
            AggregatedOperation::ExecuteBlocks(BlocksExecuteOperation {
                blocks: vec![block],
            })
        }
    }
}
