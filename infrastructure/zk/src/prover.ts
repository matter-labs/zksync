import { Command } from 'commander';
import * as utils from './utils';
import { spawn, ChildProcess } from 'child_process';

export async function prover(totalProvers: number = 1) {
    let children: ChildProcess[] = [];
    for (let id = 1; id <= totalProvers; id++) {
        const name = `${process.env.HOSTNAME}_${id}_blocks`;
        console.log('Started prover', name);
        const child = spawn(
            `cargo run --release --bin plonk_step_by_step_prover ${name}`,
            { shell: true, stdio: 'inherit' }
        );
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
    .action(async () => await prover());
