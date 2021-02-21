import { Command } from 'commander';
import * as utils from '../utils';
import path from 'path';
import fs from 'fs';

function verfiyKeysTarball() {
    const accountTreeDepth = process.env.CHAIN_CIRCUIT_ACCOUNT_TREE_DEPTH;
    const balanceTreeDepth = process.env.CHAIN_CIRCUIT_BALANCE_TREE_DEPTH;
    const keyDir = path.basename(process.env.CHAIN_CIRCUIT_KEY_DIR as string);
    return `verify-keys-${keyDir}-account-${accountTreeDepth}_-balance-${balanceTreeDepth}.tar.gz`;
}

export async function gen(command: 'contract' | 'all' | 'circuit-size') {
    const accountTreeDepth = process.env.CHAIN_CIRCUIT_ACCOUNT_TREE_DEPTH;
    const balanceTreeDepth = process.env.CHAIN_CIRCUIT_BALANCE_TREE_DEPTH;
    const keyDir = process.env.CHAIN_CIRCUIT_KEY_DIR;
    const outputDir = `${keyDir}/account-${accountTreeDepth}_balance-${balanceTreeDepth}`;

    if (command == 'all') {
        const time = new Date();
        fs.utimesSync('core/lib/crypto/src/params.rs', time, time);
        fs.mkdirSync(outputDir, { recursive: true });
        await utils.spawn('cargo run --bin key_generator --release -- keys');
        await utils.spawn('cargo run --bin key_generator --release -- contract');
    } else if (command == 'contract') {
        await utils.spawn('cargo run --bin key_generator --release -- contract');
    } else if (command == 'circuit-size') {
        await utils.spawn('cargo run --bin key_generator --release -- circuit-size');
    }

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
    fs.mkdirSync('keys/packed', { recursive: true });
    await utils.exec(`tar cvzf keys/packed/${keysTarball}  ${process.env.CHAIN_CIRCUIT_KEY_DIR}/*`);
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
        if (command != 'all' && command != 'contract' && command != 'circuit-size') {
            throw new Error(
                'Can only generate "all" or "contract" keys, or "circuit-size" for circuit size estimation'
            );
        }
        await gen(command);
    });
