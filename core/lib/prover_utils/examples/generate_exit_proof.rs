//! Generate exit proof for exodus mode given account and token
//! correct verified state should be present in the db (could be restored using `data-restore` module)

use serde::Serialize;
use std::time::Instant;
use structopt::StructOpt;
use zksync_crypto::params::MIN_NFT_TOKEN_ID;
use zksync_crypto::proof::EncodedSingleProof;
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::{block::Block, AccountId, Address, BlockNumber, TokenId, TokenLike, H256, NFT};
use zksync_utils::BigUintSerdeWrapper;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct StoredBlockInfo {
    block_number: BlockNumber,
    priority_operations: u64,
    pending_onchain_operations_hash: H256,
    timestamp: u64,
    state_hash: H256,
    commitment: H256,
}

impl StoredBlockInfo {
    pub fn from_block(block: &Block) -> Self {
        Self {
            block_number: block.block_number,
            priority_operations: block.number_of_processed_prior_ops(),
            pending_onchain_operations_hash: block.get_onchain_operations_block_info().1,
            timestamp: block.timestamp,
            state_hash: block.get_eth_encoded_root(),
            commitment: block.block_commitment,
        }
    }
}

struct NFTInfo {
    creator_id: AccountId,
    creator_address: Address,
    serial_id: u32,
    content_hash: H256,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExitProofData {
    stored_block_info: StoredBlockInfo,
    owner: Address,
    account_id: AccountId,
    token_id: TokenId,
    amount: BigUintSerdeWrapper,
    nft_creator_id: AccountId,
    nft_creator_address: Address,
    nft_serial_id: u32,
    nft_content_hash: H256,
    proof: EncodedSingleProof,
    token_address: Address,
}

#[derive(StructOpt)]
#[structopt(
    name = "zkSync operator node",
    author = "Matter Labs",
    rename_all = "snake_case"
)]
struct Opt {
    /// Account address
    #[structopt(long)]
    address: Address,

    /// Token to withdraw - "ETH" or address of the ERC20 token
    #[structopt(long)]
    token: String,
}

async fn get_nft_info(storage: &mut StorageProcessor<'_>, nft: Option<NFT>) -> NFTInfo {
    match nft {
        Some(nft) => NFTInfo {
            creator_id: nft.creator_id,
            creator_address: nft.creator_address,
            serial_id: nft.serial_id,
            content_hash: nft.content_hash,
        },
        None => {
            // The placeholder creator address should be the address
            // of the account with id 0
            let creator_address = storage
                .chain()
                .account_schema()
                .account_address_by_id(AccountId(0))
                .await
                .expect("Failed to get zero account address")
                .expect("Account with id 0 does not exist");

            NFTInfo {
                creator_id: AccountId(0),
                creator_address,
                serial_id: 0,
                content_hash: Default::default(),
            }
        }
    }
}

#[tokio::main]
async fn main() {
    vlog::init();

    let opt = Opt::from_args();

    let address = opt.address;
    let token = TokenLike::parse(&opt.token);

    let timer = Instant::now();
    vlog::info!("Restoring state from db");
    let connection_pool = ConnectionPool::new(Some(1));
    let mut storage = connection_pool
        .access_storage()
        .await
        .expect("Storage access failed");

    let token_info = storage
        .tokens_schema()
        .get_token(token)
        .await
        .expect("Db access fail")
        .expect(
            "Token not found. If you're addressing an ERC-20 token by it's symbol, \
              it may not be available after data restore. Try using token address in that case",
        );
    let token_id = token_info.id;
    let token_address = token_info.address;

    let account_id = storage
        .chain()
        .account_schema()
        .account_id_by_address(address)
        .await
        .expect("Db access fail")
        .unwrap_or_else(|| panic!("Unable to find account ID for address: {}", address));

    let accounts = storage
        .chain()
        .state_schema()
        .load_verified_state()
        .await
        .expect("Failed to load verified state")
        .1;

    let latest_block = storage
        .chain()
        .block_schema()
        .get_last_verified_confirmed_block()
        .await
        .expect("Db access fail");
    let block = storage
        .chain()
        .block_schema()
        .get_block(latest_block)
        .await
        .expect("Db access fail")
        .expect("Block not stored");
    let stored_block_info = StoredBlockInfo::from_block(&block);

    vlog::info!("Restored state from db: {} s", timer.elapsed().as_secs());

    let (proof, amount, nft) = if token_id.0 < MIN_NFT_TOKEN_ID {
        let (proof, amount) = zksync_prover_utils::exit_proof::create_exit_proof_fungible(
            accounts, account_id, address, token_id,
        )
        .expect("Failed to generate exit proof");

        (proof, amount, None)
    } else {
        let nft = storage
            .tokens_schema()
            .get_nft(token_id)
            .await
            .expect("Db access fail")
            .expect("NFT token should exist");
        let (proof, amount) = zksync_prover_utils::exit_proof::create_exit_proof_nft(
            accounts,
            account_id,
            address,
            token_id,
            nft.creator_id,
            nft.serial_id,
            nft.content_hash,
        )
        .expect("Failed to generate exit proof");

        (proof, amount, Some(nft))
    };

    let nft_info = get_nft_info(&mut storage, nft).await;

    let proof_data = ExitProofData {
        stored_block_info,
        owner: address,
        token_id,
        account_id,
        nft_creator_id: nft_info.creator_id,
        nft_creator_address: nft_info.creator_address,
        nft_serial_id: nft_info.serial_id,
        nft_content_hash: nft_info.content_hash,
        amount: amount.into(),
        proof,
        token_address,
    };

    println!("\n\n");
    println!("==========================");
    println!("Generating proof completed");
    println!("Below you can see the input data for the exit transaction on zkSync contract");
    println!("Look up the manuals of your desired smart wallet in order to know how to sign and send this transaction to the Ethereum");
    println!("==========================");

    println!("Exit transaction inputs:");

    println!(
        "{}",
        serde_json::to_string_pretty(&proof_data).expect("proof data serialize")
    );
}
