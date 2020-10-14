#!/usr/bin/env node

import { Command } from 'commander';
import { spawn } from './sh';
import fs from 'fs';
import dotenv from 'dotenv';

function loadEnv() {
    const current = 'etc/env/current';
    const ZKSYNC_ENV = process.env.ZKSYNC_ENV 
        || (fs.existsSync(current) ? fs.readFileSync(current).toString() : 'dev');
    const ENV_FILE = `etc/env/${ZKSYNC_ENV}.env`;
    if (ZKSYNC_ENV == 'dev' && !fs.existsSync('etc/env/dev.env')) {
        fs.copyFileSync('etc/env/dev.env.example', 'etc/env/dev.env');
    }
    if (!fs.existsSync(ENV_FILE)) {
        throw new Error('ZkSync config file not found: ' + ENV_FILE);
    }
    process.env.ZKSYNC_ENV = ZKSYNC_ENV;
    process.env.ENV_FILE = ENV_FILE;
    dotenv.config({ path: ENV_FILE });
}

async function main() {
    const ZKSYNC_HOME = process.env.ZKSYNC_HOME;

    if (!ZKSYNC_HOME) {
        throw new Error('Please set $ZKSYNC_HOME to the root of ZkSync repo!');
    } else {
        process.chdir(ZKSYNC_HOME);
    }

    const program = new Command();

    program
        .version('0.1.0')
        .name('zk')
        .description('ZkSync workflow tools');

    program
        .command('server')
        .description('start zksync server')
        .option('--genesis', 'generate genesis data via server')
        .action(async () => {
            loadEnv();
            await spawn('cargo run --bin zksync_server --release');
        });

    await program.parseAsync(process.argv);
}


main()
    .then(() => process.exit(0))
    .catch((err: Error) => {
        console.error(err);
        process.exit(1);
    });
