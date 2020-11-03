import { Command } from 'commander';
import * as utils from '../utils';
import path from 'path';
import fs from 'fs';

function verfiyKeysTarball() {
    const accountTreeDepth = process.env.ACCOUNT_TREE_DEPTH;
    const balanceTreeDepth = process.env.BALANCE_TREE_DEPTH;
    const keyDir = path.basename(process.env.KEY_DIR as string);
    return `verify-keys-${keyDir}-account-${accountTreeDepth}_-balance-${balanceTreeDepth}.tar.gz`;
}

export async function gen(command: 'contract' | 'all') {
    const accountTreeDepth = process.env.ACCOUNT_TREE_DEPTH;
    const balanceTreeDepth = process.env.BALANCE_TREE_DEPTH;
    const keyDir = process.env.KEY_DIR;
    const outputDir = `${keyDir}/account-${accountTreeDepth}_balance-${balanceTreeDepth}`;

    if (command == 'all') {
        const time = new Date();
        fs.utimesSync('core/models/src/lib.rs', time, time);
        fs.mkdirSync(outputDir, { recursive: true });
        await utils.spawn('cargo run --bin key_generator --release -- keys');
    }

    await utils.spawn('cargo run --bin key_generator --release -- contract');
    fs.copyFileSync(`${outputDir}/KeysWithPlonkVerifier.sol`, 'contracts/contracts/KeysWithPlonkVerifier.sol');
}

export async function unpack() {
    const keysTarball = verfiyKeysTarball();
    if (!fs.existsSync(`keys/packed/${keysTarball}`)) {
        throw new Error(`Keys file ${keysTarball} not found`);
    }
    await utils.exec(`tar xf keys/packed/${keysTarball}`);
    console.log('Keys unpacked');
}

export async function pack() {
    const keysTarball = verfiyKeysTarball();
    await utils.exec(`tar cvzf ${keysTarball}`);
    fs.mkdirSync('keys/packed', { recursive: true });
    fs.renameSync(keysTarball, `keys/packed/${keysTarball}`);
    console.log('Keys packed');
}

export const command = new Command('verify-keys').description('manage verification keys');

command.command('pack').description('reverse of unpack').action(pack);
command.command('unpack').description('unpacks verification keys for your current circuit parameters').action(unpack);

command
    .command('gen [contract|all]')
    .description('generate verification keys')
    .action(async (command?: string) => {
        command = command || 'all';
        if (command != 'all' && command != 'contract') {
            throw new Error('Can only generate "all" or "contract" keys');
        }
        await gen(command);
    });
