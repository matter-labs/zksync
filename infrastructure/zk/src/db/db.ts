import { Command } from 'commander';
import * as utils from '../utils';
import fs from 'fs';

import * as insert from './insert';
import * as update from './update';

export { insert, update };

const SQL = () => `psql "${process.env.DATABASE_URL}" -c`;

export async function reset() {
    await wait();
    await drop();
    await setup();
    await insert.contract();
    await insert.ethData();
}

export async function drop() {
    console.log('Dropping DB...');
    await utils.exec(`${SQL()} 'DROP OWNED BY CURRENT_USER CASCADE' ||
                     (${SQL()} 'DROP SCHEMA IF EXISTS public CASCADE' && ${SQL()} 'CREATE SCHEMA public')`);
}

export async function migrate() {
    console.log('Running migrations...');
    await utils.exec('cd core/storage && diesel migration run');
}

export async function setup() {
    // force read env
    delete process.env.ZKSYNC_ENV;
    utils.loadEnv();

    process.chdir('core/lib/storage');
    console.log(`DATABASE_URL = ${process.env.DATABASE_URL}`);
    await utils.exec('diesel database setup');
    await utils.exec('diesel migration run');
    fs.unlinkSync('src/schema.rs.generated');
    await utils.spawn('cargo sqlx prepare --check || cargo sqlx prepare');
    process.chdir(process.env.ZKSYNC_HOME as string);
}

export async function updateToken(token: string, symbol: string) {
    console.log(`Setting token ${token} symbol to ${symbol}`);
    await utils.exec(`${SQL()} "UPDATE tokens SET symbol = '${symbol}' WHERE address = '${token}'"`);
}

export async function wait(tries: number = 4) {
    for (let i = 0; i < tries; i++) {
        const result = await utils.allowFail(utils.exec(`pg_isready -d "${process.env.DATABASE_URL}"`));
        if (result !== null) return;  // null means failure
        await utils.sleep(5);
    }
    await utils.exec(`pg_isready -d "${process.env.DATABASE_URL}"`);
}

const dropCommand = new Command('drop')
    .description('drop the database')
    .action(drop);

const migrateCommand = new Command('migrate')
    .description('run migrations')
    .action(migrate);

const setupCommand = new Command('setup')
    .description('initialize the database and perform migrations')
    .action(setup);

const waitCommand = new Command('wait')
    .description('wait for database to get ready for interaction')
    .action(wait);

const resetCommand = new Command('reset')
    .description('reinitialize the database')
    .action(reset);

export const command = new Command('db')
    .description('database management')
    .addCommand(dropCommand)
    .addCommand(migrateCommand)
    .addCommand(setupCommand)
    .addCommand(update.command)
    .addCommand(insert.command)
    .addCommand(waitCommand)
    .addCommand(resetCommand);

