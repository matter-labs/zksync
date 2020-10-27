import { Command } from 'commander';
import * as utils from '../utils';
import * as db from '../db/db';

export async function rootHash() {
    const query = `
        WITH last_block (number) AS (
            SELECT max(block_number) 
            FROM operations 
            WHERE action_type = 'VERIFY' and confirmed = true
        ) 
        SELECT encode(root_hash, 'hex') 
        FROM blocks, last_block 
        WHERE blocks.number = last_block.number;`;
    const { stdout: blockHash } = await utils.exec(`echo "${query}" | psql "${process.env.DATABASE_URL}" -t`);
    if (blockHash.trim() == '') {
        throw new Error('Unable to load the latest block hash');
    }
    return blockHash.trim();
}

export async function restart() {
    await db.reset();
    await utils.spawn('cargo run --bin zksync_data_restore --release -- --genesis --finite');
}

export async function resume() {
    await utils.spawn('cargo run --bin zksync_data_restore --release -- --continue');
}

export async function run() {
    await utils.spawn('cargo run --bin zksync_data_restore --release -- --genesis --finite');
}

export async function check(expectedHash: string) {
    await db.reset();
    await utils.spawn(
        `cargo run --bin zksync_data_restore --release -- --genesis --finite --final_hash ${expectedHash}`
    );
}

export async function checkExisting() {
    const expectedHash = await rootHash();
    await check(expectedHash);
}

export const command = new Command('data-restore');

command.command('restart').description('wipe the database and run data restore in finite mode').action(restart);
command.command('resume').description('run data restore in "resume" mode').action(resume);
command.command('run').description('do not wipe the database and run data restore in finite mode').action(run);

command
    .command('check <hash>')
    .description('wipe the database, run the data restore in finite mode and check the root hash')
    .action(check);

command
    .command('check-existing')
    .description(`like "check", but instead hash is loaded from the database before wiping it`)
    .action(checkExisting);

command
    .command('root-hash')
    .description('find the hash of the latest verified block and print it')
    .action(async () => {
        console.log(await rootHash());
    });
