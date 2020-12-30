import { Command } from 'commander';
import * as utils from './utils';
import * as env from './env';
import fs from 'fs';

import * as db from './db/db';
import * as run from './run/run';

export function prepareVerify() {
    const keyDir = process.env.KEY_DIR;
    const accountTreeDepth = process.env.ACCOUNT_TREE_DEPTH;
    const balanceTreeDepth = process.env.BALANCE_TREE_DEPTH;
    const source = `${keyDir}/account-${accountTreeDepth}_balance-${balanceTreeDepth}/KeysWithPlonkVerifier.sol`;
    const dest = 'contracts/contracts/KeysWithPlonkVerifier.sol';
    try {
        fs.copyFileSync(source, dest);
    } catch (err) {
        console.error('Please download the keys');
        throw err;
    }
}

export async function build() {
    await utils.confirmAction();
    prepareVerify();
    await utils.spawn('yarn contracts build');
}

export async function publish() {
    await utils.spawn('yarn contracts publish-sources');
}

export async function deploy() {
    await utils.confirmAction();
    console.log('Deploying contracts, results will be inserted into the db');
    await utils.spawn('yarn contracts deploy-no-build | tee deploy.log');
    const deployLog = fs.readFileSync('deploy.log').toString();
    const envVars = [
        'GOVERNANCE_TARGET_ADDR',
        'VERIFIER_TARGET_ADDR',
        'CONTRACT_TARGET_ADDR',
        'GOVERNANCE_ADDR',
        'CONTRACT_ADDR',
        'VERIFIER_ADDR',
        'GATEKEEPER_ADDR',
        'DEPLOY_FACTORY_ADDR',
        'GENESIS_TX_HASH'
    ];
    for (const envVar of envVars) {
        const pattern = new RegExp(`${envVar}=.*`, 'g');
        const matches = deployLog.match(pattern);
        if (matches !== null) {
            env.modify(envVar, matches[0]);
        }
    }
}

export async function redeploy() {
    await deploy();
    await db.insert.contract();
    await run.governanceAddERC20('dev');
    await publish();
}

export const command = new Command('contract').description('contract management');

command.command('prepare-verify').description('initialize verification keys for contracts').action(prepareVerify);
command.command('redeploy').description('redeploy contracts and update addresses in the db').action(redeploy);
command.command('deploy').description('deploy contracts').action(deploy);
command.command('build').description('build contracts').action(build);
command.command('publish').description('publish contracts').action(publish);
