import { Command } from 'commander';
import * as utils from './utils';
import fs from 'fs';

import * as db from './db/db';
import * as server from './server';
import * as contract from './contract';
import * as run from './run/run';
import { up } from './up';

export async function init() {
    await checkEnv();
    if (!process.env.CI) {
        await up();
    }
    await utils.allowFail(yarn());
    await downloadMissingKeys();
    await run.verifyKeys.unpack();
    await db.setup();
    await contract.buildDev();
    await deployERC20('dev');
    await contract.build();
    await db.reset();
    await server.genesis();
    await contract.redeploy();
}

async function deployERC20(command: 'dev' | 'new', name?: string, symbol?: string, decimals?: string) {
    if (command == 'dev') {
        await utils.exec(`yarn --silent --cwd contracts deploy-erc20 add-multi '
            [
                { "name": "DAI",  "symbol": "DAI",  "decimals": 18 },
                { "name": "wBTC", "symbol": "wBTC", "decimals":  8 },
                { "name": "BAT",  "symbol": "BAT",  "decimals": 18 },
                { "name": "MLTT", "symbol": "MLTT", "decimals": 18 }
            ]' > ./etc/tokens/localhost.json`);
    } else if (command == 'new') {
        await utils.exec(
            `yarn --cwd contracts deploy-erc20 add --name ${name} --symbol ${symbol} --decimals ${decimals}`
        );
    }
}

// installs all dependencies and builds our js packages
async function yarn() {
    await utils.spawn('yarn --cwd sdk/zksync.js');
    await utils.spawn('yarn --cwd sdk/zksync.js build');
    await utils.spawn('yarn --cwd contracts');
    await utils.spawn('yarn --cwd core/tests/ts-tests');
    await utils.spawn('yarn --cwd infrastructure/explorer');
    await utils.spawn('yarn --cwd infrastructure/fee-seller');
    await utils.spawn('yarn --cwd infrastructure/zcli');
    await utils.spawn('yarn --cwd infrastructure/analytics');
}

async function checkEnv() {
    await utils.exec('which node');
    const { stdout: version } = await utils.exec('node --version');
    if ('v10.20' >= version) {
        throw new Error('Error, node.js version 10.20.1 or higher is required');
    }
    await utils.exec('which yarn');
    await utils.exec('which docker');
    await utils.exec('which docker-compose');
    await utils.exec('which cargo');
    await utils.exec('cargo sqlx --version');
    await utils.exec('which psql');
    await utils.exec('which pg_isready');
    await utils.exec('which diesel');
    await utils.exec('which solc');
}

async function downloadMissingKeys() {
    const URL = 'https://universal-setup.ams3.digitaloceanspaces.com';
    fs.mkdirSync('keys/setup', { recursive: true });
    process.chdir('keys/setup');
    for (let power = 20; power <= 26; power++) {
        if (!fs.existsSync(`setup_2^${power}.key`)) {
            await utils.spawn(`axel -c ${URL}/setup_2%5E${power}.key`);
            await utils.sleep(1);
        }
    }
    process.chdir('../..');
}

export const command = new Command('init')
    .description('perform zksync network initialization for development')
    .action(init);
