import { Command } from 'commander';
import * as utils from './utils';
import * as env from './env';
import fs from 'fs';
import * as db from './db/db';

export async function server() {
    await utils.spawn('cargo run --bin zksync_server --release');
}

export async function genesis() {
    await db.reset();
    await utils.confirmAction();
    await utils.spawn('cargo run --bin zksync_server --release -- --genesis | tee genesis.log');
    const genesisRoot = fs.readFileSync('genesis.log').toString();
    const date = new Date();
    const [year, month, day, hour, minute, second] = [
        date.getFullYear(),
        date.getMonth(),
        date.getDate(),
        date.getHours(),
        date.getMinutes(),
        date.getSeconds()
    ];
    const label = `${process.env.ZKSYNC_ENV}-Genesis_gen-${year}-${month}-${day}-${hour}${minute}${second}`;
    fs.mkdirSync(`logs/${label}`, { recursive: true });
    fs.copyFileSync('genesis.log', `logs/${label}/genesis.log`);
    env.modify('GENESIS_ROOT', genesisRoot);
}

export const command = new Command('server')
    .description('start zksync server')
    .option('--genesis', 'generate genesis data via server')
    .action(async (cmd: Command) => {
        if (cmd.genesis) {
            await genesis();
        } else {
            await server();
        }
    });
