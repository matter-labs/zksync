#!/usr/bin/env node

import { program, Command } from 'commander';
import { spawnSync } from 'child_process';
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
import { command as fmt } from './fmt';
import { command as lint } from './lint';
import { command as completion } from './completion';
import { command as config } from './config';
import { command as apiDocs } from './api-docs';
import * as env from './env';

const COMMANDS = [
    server,
    up,
    down,
    db,
    contract,
    dummyProver,
    init,
    kube,
    prover,
    run,
    test,
    fmt,
    lint,
    docker,
    config,
    apiDocs,
    env.command,
    completion(program as Command)
];

async function main() {
    const cwd = process.cwd();
    const ZKSYNC_HOME = process.env.ZKSYNC_HOME;

    if (!ZKSYNC_HOME) {
        throw new Error('Please set $ZKSYNC_HOME to the root of ZkSync repo!');
    } else {
        process.chdir(ZKSYNC_HOME);
    }

    await env.load();

    program.version('0.1.0').name('zk').description('zksync workflow tools');

    for (const command of COMMANDS) {
        program.addCommand(command);
    }

    // f command is special-cased because it is necessary
    // for it to run from $PWD and not from $ZKSYNC_HOME
    program
        .command('f <command...>')
        .allowUnknownOption()
        .action((command: string[]) => {
            process.chdir(cwd);
            const result = spawnSync(command[0], command.slice(1), { stdio: 'inherit' });
            if (result.error) {
                throw result.error;
            }
            process.exitCode = result.status || undefined;
        });

    await program.parseAsync(process.argv);
}

main().catch((err: Error) => {
    console.error('Error:', err.message || err);
    process.exitCode = 1;
});
