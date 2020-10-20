import { Command } from 'commander';
import * as utils from './utils';
import fs from 'fs';

import * as server from './server';
import * as contract from './contract';
import * as db from './db/db';

const VERIFIER_FILE = 'contracts/contracts/Verifier.sol';

async function performRedeployment() {
    await contract.build();
    await db.reset();
    await server.genesis();
    await contract.redeploy();
}

export async function run() {
    await utils.spawn("cargo run --release --bin dummy_prover dummy-prover-instance");
}

export async function status() {
    try {
        // using grep and not native fs.readFile because grep -l stops after first match
        await utils.exec(`grep -l 'constant DUMMY_VERIFIER = true' ${VERIFIER_FILE}`);
        console.log("Dummy Prover status: enabled");
        return true;
    } catch (err) {
        console.log("Dummy Prover status: disabled");
        return false;
    }
}

async function toggle(from: string, to: string) {
    const verifierSource = fs.readFileSync(VERIFIER_FILE).toString();
    const replaced = verifierSource.replace(`constant DUMMY_VERIFIER = ${from}`, `constant DUMMY_VERIFIER = ${to}`);
    fs.writeFileSync(VERIFIER_FILE, replaced);
    await status();
    console.log("Redeploying the contract...");
    await performRedeployment();
    console.log("Done.")
}

export async function enable() {
    await toggle("false", "true");
}

export async function disable() {
    await toggle("true", "false");
}

export const command = new Command('dummy-prover')
    .description('commands for zksync dummy prover');

command
    .command('run')
    .description('launch the dummy prover')
    .action(run);

command
    .command('status')
    .description('check if dummy prover is enabled')
    // @ts-ignore
    .action(status);

command
    .command('enable')
    .description('enable the dummy prover')
    .action(enable);

command
    .command('disable')
    .description('disable the dummy prover')
    .action(disable);

