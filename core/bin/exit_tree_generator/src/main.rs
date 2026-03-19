use structopt::StructOpt;
use web3::ethabi::Address;
#[cfg(feature = "postgres")]
use zksync_exit_tree_generator::restore_tree_from_db;
use zksync_exit_tree_generator::{keccak_merkle_tree, token_id_restorer, zksync_tree};

#[derive(StructOpt)]
#[structopt(name = "Exit tree generator", author = "Matter Labs")]
enum Opt {
    /// Restore token IDs from Ethereum events
    RestoreTokenIds {
        /// Web3 URL to connect to Ethereum node
        #[structopt(long = "web3")]
        web3_url: Option<String>,

        /// Provides path to the contracts config file
        #[structopt(long = "config")]
        config_path: Option<String>,
    },
    /// Restore ZKSYNC Merkle tree from account and balance CSV files
    RestoreZksyncTree {
        /// Path to the CSV file with accounts.
        #[structopt(long = "accounts")]
        accounts: String,

        /// Path to the balances CSV file
        #[structopt(long = "balances")]
        balances: String,
    },
    ///  Generate new leaves for keccak Merkle tree from account/balance/token CSV files
    CreateNewLeaves {
        /// Path to the CSV file with accounts
        #[structopt(long = "accounts")]
        accounts: String,
        /// Path to the CSV file with balances
        #[structopt(long = "balances")]
        balances: String,
        /// Path to the CSV file with tokens
        #[structopt(long = "tokens")]
        tokens: String,
        /// Path to the output CSV file with new leaves. Optional. If not provided, defaults to `new_leaves.csv`
        #[structopt(long = "output")]
        output: Option<String>,
    },
    /// Restore Merkle tree from verified database state
    #[cfg(feature = "postgres")]
    RestoreTreeFromDb,
    /// Create Merkle proof for a specific account and multiple tokens
    CreateProof {
        /// Account address to create the proof for
        #[structopt(long = "account")]
        account: Address,
        /// Token addresses to create the proof for
        #[structopt(long = "tokens")]
        tokens: Vec<Address>,
        /// Path to the CSV file with leaves. Optional. If not provided, defaults to `new_leaves.csv`
        leaves_path: Option<String>,
    },
    /// Calculate Merkle root hash from leaves stored in a CSV file
    CalculateRootForKeccakTree {
        /// Path to the CSV file with leaves. Optional. If not provided, defaults to `new_leaves.csv`
        leaves_path: Option<String>,
    },
}

/// Main entry point for the exit tree generator tool.
/// Parses command-line arguments and executes the appropriate subcommand.
fn main() -> anyhow::Result<()> {
    println!("Exit tree generator tool");
    let opt = Opt::from_args();

    match opt {
        Opt::RestoreTokenIds {
            web3_url,
            config_path,
        } => {
            token_id_restorer::run(web3_url, config_path)?;
        }
        Opt::RestoreZksyncTree { accounts, balances } => {
            zksync_tree::restore_zksync_tree_from_files(&accounts, &balances)?;
        }
        Opt::CreateNewLeaves {
            accounts,
            balances,
            tokens,
            output,
        } => {
            keccak_merkle_tree::run_create_keccak_leaves(accounts, balances, tokens, output)?;
        }
        #[cfg(feature = "postgres")]
        Opt::RestoreTreeFromDb => {
            restore_tree_from_db::run_restore_tree_from_db()?;
        }
        Opt::CreateProof {
            account,
            tokens,
            leaves_path,
        } => {
            keccak_merkle_tree::run_create_proof_for_keccak_tree(account, &tokens, leaves_path)?;
        }
        Opt::CalculateRootForKeccakTree { leaves_path } => {
            keccak_merkle_tree::run_calculate_root_for_keccak_tree(leaves_path)?;
        }
    }
    Ok(())
}
