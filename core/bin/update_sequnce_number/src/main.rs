use std::cmp;
use structopt::StructOpt;
use zksync_storage::StorageProcessor;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(long)]
    last_block_for_update: i64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    let mut storage = StorageProcessor::establish_connection().await?;
    let block_range = 200;
    let mut start_block = storage
        .chain()
        .operations_ext_schema()
        .min_block_for_update_sequence()
        .await;

    let mut to_block = cmp::min(start_block + block_range, opt.last_block_for_update);
    let mut last_known_sequence_number = None;
    while to_block <= opt.last_block_for_update {
        println!("Start updating from {:?} to {:?}", start_block, to_block);
        last_known_sequence_number = Some(
            storage
                .chain()
                .operations_ext_schema()
                .update_sequence_number_for_blocks(
                    start_block,
                    to_block,
                    last_known_sequence_number,
                )
                .await,
        );
        start_block = to_block + 1;
        to_block = cmp::min(start_block + block_range, opt.last_block_for_update);
    }
    Ok(())
}
