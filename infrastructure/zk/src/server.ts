import { Command } from 'commander';
import * as utils from './utils';
import fs from 'fs';

export async function server() {
    await utils.spawn('cargo run --bin zksync_server --release');
}

export async function genesis() {
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
    // const envFile = process.env.ENV_FILE as string;
    // const env = fs.readFileSync(envFile).toString();
    // fs.writeFileSync(envFile, env.replace(/GENESIS_ROOT=.*/g, genesisRoot));
    utils.modifyEnv('GENESIS_ROOT', genesisRoot);
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
