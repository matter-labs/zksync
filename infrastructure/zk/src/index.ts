#!/usr/bin/env node

import { program } from 'commander';
import { command as server } from './server';
import { command as up } from './up';
import { command as down } from './down';
import { command as db } from './db';
import * as utils from './utils';

async function main() {
    const ZKSYNC_HOME = process.env.ZKSYNC_HOME;

    if (!ZKSYNC_HOME) {
        throw new Error('Please set $ZKSYNC_HOME to the root of ZkSync repo!');
    } else {
        process.chdir(ZKSYNC_HOME);
    }

    utils.loadEnv();

    program
        .version('0.1.0')
        .name('zk')
        .description('ZkSync workflow tools')
        .addCommand(server)
        .addCommand(up)
        .addCommand(down)
        .addCommand(db);

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err: Error) => {
        console.error(err);
        process.exit(1);
    });
