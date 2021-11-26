import { Command } from 'commander';
import chalk from 'chalk';
import * as utils from './utils';

import * as db from './db/db';
import * as server from './server';
import * as contract from './contract';
import * as run from './run/run';
import * as env from './env';
import * as docker from './docker';
import { up } from './up';

const entry = chalk.bold.yellow;
const announce = chalk.yellow;
const success = chalk.green;
const timestamp = chalk.grey;

export async function init() {
    await announced('Creating docker volumes', createVolumes());
    if (!process.env.CI) {
        await announced('Pulling images', docker.pull());
        await announced('Checking environment', checkEnv());
        await announced('Checking git hooks', env.gitHooks());
        await announced('Setting up containers', up());
    }
    await announced('Compiling JS packages', run.yarn());
    await announced('Checking PLONK setup', run.plonkSetup());
    await announced('Unpacking verification  keys', run.verifyKeys.unpack());
    await announced('Setting up database', db.setup());
    await announced('Building contracts', contract.build());
    await announced('Deploying localhost ERC20 tokens', run.deployERC20('dev'));
    await announced('Deploying localhost EIP1271 contract', run.deployEIP1271());
    await announced('Deploying withdrawal helpers contracts', run.deployWithdrawalHelpersContracts());
    await announced('Running server genesis setup', server.genesis());
    await announced('Deploying main contracts', contract.redeploy());
    if (!process.env.CI) {
        await announced('Restarting dev liquidity watcher', docker.restart('dev-liquidity-token-watcher'));
    }
}

// Wrapper that writes an announcement and completion notes for each executed task.
async function announced(fn: string, promise: Promise<void>) {
    const announceLine = `${entry('>')} ${announce(fn)}`;
    const separator = '-'.repeat(fn.length + 2); // 2 is the length of "> ".
    console.log(`\n` + separator); // So it's easier to see each individual step in the console.
    console.log(announceLine);

    const start = new Date().getTime();
    // The actual execution part
    await promise;

    const time = new Date().getTime() - start;
    const successLine = `${success('✔')} ${fn} done`;
    const timestampLine = timestamp(`(${time}ms)`);
    console.log(`${successLine} ${timestampLine}`);
}

async function createVolumes() {
    await utils.exec('mkdir -p $ZKSYNC_HOME/volumes/geth');
    await utils.exec('mkdir -p $ZKSYNC_HOME/volumes/postgres');
    await utils.exec('mkdir -p $ZKSYNC_HOME/volumes/tesseracts');
}

async function checkEnv() {
    const tools = ['node', 'yarn', 'docker', 'docker-compose', 'cargo', 'psql', 'pg_isready', 'diesel'];
    for (const tool of tools) {
        await utils.exec(`which ${tool}`);
    }
    await utils.exec('cargo sqlx --version');
    const { stdout: version } = await utils.exec('node --version');
    // Node v14.14 is required because
    // the `fs.rmSync` function was added in v14.14.0
    if ('v14.14' >= version) {
        throw new Error('Error, node.js version 14.14.0 or higher is required');
    }
}

export const command = new Command('init')
    .description('perform zksync network initialization for development')
    .action(init);
