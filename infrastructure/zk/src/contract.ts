import { Command } from 'commander';
import * as utils from './utils';
import fs from 'fs';

import * as db from './db/db';

export function prepareVerify() {
    const keyDir = process.env.KEY_DIR;
    const accountTreeDepth = process.env.ACCOUNT_TREE_DEPTH;
    const balanceTreeDepth = process.env.BALANCE_TREE_DEPTH;
    const source = `${keyDir}/account-${accountTreeDepth}_balance-${balanceTreeDepth}/KeysWithPlonkVerifier.sol`;
    const dest = 'contracts/contracts/KeysWithPlonkVerifier.sol';
    try {
        fs.copyFileSync(source, dest);
    } catch (err) {
        console.error("Please download keys");
        throw err;
    }
}

export async function build() {
    await utils.spawn('cargo run --release --bin gen_token_add_contract');
    await utils.spawn('yarn --cwd contracts build')
}

export async function buildDev() {
    // TODO: prepare-test-contracts.sh
    await utils.spawn('yarn --cwd contracts build-dev');
}

export async function publish() {
    await utils.spawn('yarn --cwd contracts publish-sources');
}

export async function deploy() {
    console.log('Redeploying contracts, results will be inserted into the db');
    await utils.spawn('yarn --cwd contracts deploy-no-build | tee deploy.log');
    const deployLog = fs.readFileSync('deploy.log').toString();
    const envVars = [
        "GOVERNANCE_TARGET_ADDR",
        "VERIFIER_TARGET_ADDR",
        "CONTRACT_TARGET_ADDR",
        "GOVERNANCE_ADDR",
        "CONTRACT_ADDR",
        "VERIFIER_ADDR",
        "GATEKEEPER_ADDR",
        "DEPLOY_FACTORY_ADDR",
        "GENESIS_TX_HASH"
    ];
    for (const envVar of envVars) {
        const pattern = new RegExp(`${envVar}=.*`, 'g');
        // @ts-ignore
        utils.modifyEnv(envVar, deployLog.match(pattern)[0]);
    }
}

export async function redeploy() {
    await deploy();
    await db.insert.contract();
    await publish();
}

export const command = new Command('contract')
    .description('contract management');

command
    .command('prepare-verify')
    .description('initialize verification keys for contracts')
    .action(prepareVerify);

command
    .command('redeploy')
    .description('redeploy contracts and update addresses in the db')
    .action(redeploy);

command
    .command('deploy')
    .description('deploy contracts')
    .action(deploy);

command
    .command('build')
    .description('build contracts')
    .action(build);

command
    .command('build-dev')
    .description('build development contracts')
    .action(buildDev);

command
    .command('publish')
    .description('publish contracts')
    .action(publish);
