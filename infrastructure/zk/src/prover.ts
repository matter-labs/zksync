import { Command } from 'commander';
import * as utils from './utils';

export async function prover(totalProvers: number) {
    let children: utils.ChildProcess[] = [];
    for (let id = 1; id <= totalProvers; id++) {
        const name = `${process.env.HOSTNAME}_${id}_blocks`;
        console.log('Started prover', name);
        const child = utils.background(`cargo run --release --bin plonk_step_by_step_prover ${name}`);
        children.push(child);
    }
    process.on('SIGINT', () => {
        console.log('Killing provers...');
        for (const child of children) {
            child.kill();
        }
    });
    while (true) {
        await utils.sleep(1000_000);
    }
}

export const command = new Command('prover')
    .description('run zksync prover')
    .arguments('[number_of_provers]')
    .action(async (number_of_provers?: number) => {
        await prover(number_of_provers || 1);
    });
