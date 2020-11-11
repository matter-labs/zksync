import { Command } from 'commander';
import * as utils from './utils';

import * as server from './server';
import * as contract from './contract';
import * as db from './db/db';

const VERIFIER_FILE = 'contracts/contracts/Verifier.sol';

async function performRedeployment() {
    await contract.build();
    await server.genesis();
    await contract.redeploy();
}

export async function run() {
    await utils.spawn('cargo run --release --bin dummy_prover dummy-prover-instance');
}

export async function status() {
    try {
        // using grep and not native fs.readFile because grep -l stops after first match
        await utils.exec(`grep -l 'constant DUMMY_VERIFIER = true' ${VERIFIER_FILE}`);
        console.log('Dummy Prover status: enabled');
        return true;
    } catch (err) {
        console.log('Dummy Prover status: disabled');
        return false;
    }
}

export async function ensureDisabled() {
    const enabled = await status();
    if (enabled) {
        throw new Error("This is not allowed, please change DUMMY_VERIFIER constant value to 'false'");
    }
}

async function setStatus(value: boolean, redeploy: boolean) {
    utils.replaceInFile(VERIFIER_FILE, '(.*constant DUMMY_VERIFIER)(.*);', `$1 = ${value};`);
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

command
    .command('ensure-disabled')
    .description('checks if dummy-prover is disabled and exits with code 1 otherwise')
    .action(ensureDisabled);
