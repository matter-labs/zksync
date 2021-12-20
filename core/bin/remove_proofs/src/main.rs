use anyhow::ensure;
use structopt::StructOpt;
use zksync_storage::StorageProcessor;
use zksync_types::BlockNumber;

async fn remove_operations(
    storage: &mut StorageProcessor<'_>,
    last_block: BlockNumber,
) -> anyhow::Result<()> {
    let mut transaction = storage.start_transaction().await?;
    transaction
        .prover_schema()
        .remove_witnesses(last_block)
        .await?;
    println!("`witness` table is cleaned");

    transaction
        .chain()
        .operations_schema()
        .remove_eth_unprocessed_aggregated_ops()
        .await?;
    println!("`eth_unprocessed_aggregated_ops` table is cleaned");
    transaction
        .chain()
        .operations_schema()
        .remove_aggregate_operations(last_block)
        .await?;
    println!("`aggregate_operations`, `eth_aggregated_ops_binding`, `eth_tx_hashes`, `eth_operations` tables are cleaned");

    transaction
        .prover_schema()
        .remove_proofs(last_block)
        .await?;
    println!("`proofs` table is cleaned");
    transaction
        .prover_schema()
        .remove_aggregated_proofs(last_block)
        .await?;
    println!("`aggregated_proofs` table is cleaned");
    transaction
        .prover_schema()
        .remove_prover_jobs(last_block)
        .await?;
    println!("`prover_job_queue` table is cleaned");

    transaction.commit().await?;

    println!("Proofs were deleted from storage");
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "zkSync proof delete tool", author = "Matter Labs")]
#[structopt(about = "Tool for deleting proofs from database")]
struct Opt {
    /// Last correct block, tool reverts blocks with numbers greater than this field.
    #[structopt(long)]
    last_correct_block: u32,
}

// TODO: don't use anyhow (ZKS-588)
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    let mut storage = StorageProcessor::establish_connection().await?;

    let last_commited_block = storage
        .chain()
        .block_schema()
        .get_last_committed_confirmed_block()
        .await?;
    let last_proven_block = storage
        .chain()
        .block_schema()
        .get_last_proven_confirmed_block()
        .await?;

    println!(
        "Last committed block {} proven {}",
        &last_commited_block, &last_proven_block
    );

    ensure!(
        *last_proven_block <= opt.last_correct_block,
        "Some proofs has already been published to ethereum"
    );

    let last_block = BlockNumber(opt.last_correct_block);

    println!("Start remove proofs");
    remove_operations(&mut storage, last_block).await?;

    Ok(())
}
