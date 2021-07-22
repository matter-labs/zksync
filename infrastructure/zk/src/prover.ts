import { Command } from 'commander';
import * as utils from './utils';
import os from 'os';

export async function prover(totalProvers: number) {
    let children: utils.ChildProcess[] = [];

    for (let id = 1; id <= totalProvers; id++) {
        const name = `${os.hostname()}_${id}_blocks`;
        console.log('Started prover', name);
        const child = utils.background(`cargo run --release --bin plonk_step_by_step_prover ${name}`);
        children.push(child);
    }

    process.on('SIGINT', () => {
        for (const child of children) {
            child.kill('SIGINT');
        }
    });
}

export const command = new Command('prover')
    .description('run zksync prover')
    .arguments('[number_of_provers]')
    .action(async (provers?: string) => {
        const totalProvers = provers ? parseInt(provers) : 1;
        await prover(totalProvers);
    });
