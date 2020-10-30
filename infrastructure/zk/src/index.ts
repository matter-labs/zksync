#!/usr/bin/env node

import { program, Command } from 'commander';
import { command as server } from './server';
import { command as up } from './up';
import { command as down } from './down';
import { command as db } from './db/db';
import { command as contract } from './contract';
import { command as dummyProver } from './dummy-prover';
import { command as init } from './init';
import { command as kube } from './kube';
import { command as prover } from './prover';
import { command as run } from './run/run';
import { command as test } from './test/test';
import { command as docker } from './docker';
import { command as completion } from './completion';
import { command as f } from './f';
import * as env from './env';

async function main() {
    const ZKSYNC_HOME = process.env.ZKSYNC_HOME;

    if (!ZKSYNC_HOME) {
        throw new Error('Please set $ZKSYNC_HOME to the root of ZkSync repo!');
    } else {
        process.chdir(ZKSYNC_HOME);
    }

    env.load();

    program
        .version('0.1.0')
        .name('zk')
        .description('zksync workflow tools')
        .addCommand(server)
        .addCommand(up)
        .addCommand(down)
        .addCommand(db)
        .addCommand(contract)
        .addCommand(dummyProver)
        .addCommand(kube)
        .addCommand(init)
        .addCommand(prover)
        .addCommand(run)
        .addCommand(test)
        .addCommand(docker)
        .addCommand(f)
        .addCommand(env.command)
        .addCommand(completion(program as Command));

    await program.parseAsync(process.argv);
}

main().catch((err: Error) => {
    console.error('Error:', err.message || err);
    process.exitCode = 1;
});
