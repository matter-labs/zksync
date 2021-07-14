import { Command } from 'commander';
import * as utils from './utils';
import * as env from './env';
import fs from 'fs';

import * as db from './db/db';
import * as run from './run/run';

export function prepareVerify() {
    const keyDir = process.env.CHAIN_CIRCUIT_KEY_DIR;
    const accountTreeDepth = process.env.CHAIN_CIRCUIT_ACCOUNT_TREE_DEPTH;
    const balanceTreeDepth = process.env.CHAIN_CIRCUIT_BALANCE_TREE_DEPTH;
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
        'CONTRACTS_GOVERNANCE_TARGET_ADDR',
        'CONTRACTS_VERIFIER_TARGET_ADDR',
        'CONTRACTS_CONTRACT_TARGET_ADDR',
        'CONTRACTS_GOVERNANCE_ADDR',
        'CONTRACTS_CONTRACT_ADDR',
        'CONTRACTS_VERIFIER_ADDR',
        'CONTRACTS_UPGRADE_GATEKEEPER_ADDR',
        'CONTRACTS_DEPLOY_FACTORY_ADDR',
        'CONTRACTS_FORCED_EXIT_ADDR',
        'CONTRACTS_NFT_FACTORY_ADDR',
        'CONTRACTS_GENESIS_TX_HASH',
        'CONTRACTS_LISTING_GOVERNANCE'
    ];
    let updatedContracts = '';
    for (const envVar of envVars) {
        const pattern = new RegExp(`${envVar}=.*`, 'g');
        const matches = deployLog.match(pattern);
        if (matches !== null) {
            const varContents = matches[0];
            env.modify(envVar, varContents);
            env.modify_contracts_toml(envVar, varContents);

            updatedContracts += `${varContents}\n`;
        }
    }

    // Write updated contract addresses and tx hashes to the separate file
    // Currently it's used by loadtest github action to update deployment configmap.
    fs.writeFileSync('deployed_contracts.log', updatedContracts);
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
