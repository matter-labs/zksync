#!/usr/bin/env node

import { Command } from 'commander';
import { sh } from './sh';
import fs from 'fs';
import dotenv from 'dotenv';

function loadEnv() {
    const ZKSYNC_HOME = process.env.ZKSYNC_HOME;
    if (!ZKSYNC_HOME) {
        throw new Error('Please set $ZKSYNC_HOME to the root of ZkSync repo!');
    }
    process.chdir(ZKSYNC_HOME);
    const current = 'etc/env/current';
    const env = fs.existsSync(current) ? fs.readFileSync(current) : 'dev';
    const envFile = `etc/env/${env}.env`;
    if (env == 'dev' && !fs.existsSync('etc/env/dev.env')) {
        fs.copyFileSync('etc/env/dev.env.example', 'etc/env/dev.env');
    }
    if (!fs.existsSync(envFile)) {
        throw new Error('ZkSync config file not found: ' + envFile);
    }
    dotenv.config({ path: envFile });
}

async function main() {
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
            // TODO: make it continuously print logs
            await sh('cargo run --bin zksync_server --release');
        })

    program.on('command:*', loadEnv);
    await program.parseAsync(process.argv);
}


main()
    .then(() => process.exit(0))
    .catch((err: Error) => {
        console.error('Error:', err.message);
        process.exit(1);
    });
