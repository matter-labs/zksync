import { Command } from 'commander';
import * as utils from './utils';

import * as server from './server';
import * as contract from './contract';
import * as env from './env';

async function performRedeployment() {
    await contract.build();
    await server.genesis();
    await contract.redeploy();
}

export async function run() {
    const child = utils.background('cargo run --release --bin dummy_prover dummy-prover-instance');
    process.on('SIGINT', () => {
        child.kill('SIGINT');
    });
}

export async function status() {
    if (process.env.CONTRACTS_TEST_DUMMY_VERIFIER == 'true') {
        console.log('Dummy Prover status: enabled');
        return true;
    }
    console.log('Dummy Prover status: disabled');
    return false;
}

async function setStatus(value: boolean, redeploy: boolean) {
    env.modify('CONTRACTS_TEST_DUMMY_VERIFIER', `CONTRACTS_TEST_DUMMY_VERIFIER="${value}"`);
    env.modify_contracts_toml('CONTRACTS_TEST_DUMMY_VERIFIER', `CONTRACTS_TEST_DUMMY_VERIFIER="${value}"`);
    await status();
    if (redeploy) {
        console.log('Redeploying the contract...');
        await performRedeployment();
        console.log('Done.');
    }
}

export async function enable(redeploy: boolean = true) {
    await setStatus(true, redeploy);
}

export async function disable(redeploy: boolean = true) {
    await setStatus(false, redeploy);
}

export const command = new Command('dummy-prover').description('commands for zksync dummy prover');

command.command('run').description('launch the dummy prover').action(run);

command
    .command('enable')
    .description('enable the dummy prover')
    .option('--no-redeploy', 'do not redeploy the contracts')
    .action(async (cmd: Command) => {
        await enable(cmd.redeploy);
    });

command
    .command('disable')
    .description('disable the dummy prover')
    .option('--no-redeploy', 'do not redeploy the contracts')
    .action(async (cmd: Command) => {
        await disable(cmd.redeploy);
    });

command
    .command('status')
    .description('check if dummy prover is enabled')
    // @ts-ignore
    .action(status);
