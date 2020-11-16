import { Command } from 'commander';
import * as utils from './utils';

import * as db from './db/db';
import * as server from './server';
import * as contract from './contract';
import * as run from './run/run';
import * as env from './env';
import { up } from './up';

export async function init() {
    if (!process.env.CI) {
        await checkEnv();
        await env.gitHooks();
        await up();
    }
    await utils.allowFail(run.yarn());
    await run.plonkSetup();
    await run.verifyKeys.unpack();
    await db.setup();
    await contract.buildDev();
    await run.deployERC20('dev');
    await run.deployEIP1271();
    await contract.build();
    await server.genesis();
    await contract.redeploy();
}

async function checkEnv() {
    const tools = ['node', 'yarn', 'docker', 'docker-compose', 'cargo', 'psql', 'pg_isready', 'diesel', 'solc'];
    for (const tool of tools) {
        await utils.exec(`which ${tool}`);
    }
    await utils.exec('cargo sqlx --version');
    const { stdout: version } = await utils.exec('node --version');
    if ('v14' >= version) {
        throw new Error('Error, node.js version 14 or higher is required');
    }
}

export const command = new Command('init')
    .description('perform zksync network initialization for development')
    .action(init);
