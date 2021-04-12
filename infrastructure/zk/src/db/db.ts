import { Command } from 'commander';
import * as utils from '../utils';
import * as env from '../env';
import fs from 'fs';

import * as insert from './insert';
import * as update from './update';

export { insert, update };

const SQL = () => `psql "${process.env.DATABASE_URL}" -c`;

export async function reset() {
    await utils.confirmAction();
    await wait();
    await drop();
    await setup();
    await insert.contract();
    await insert.ethData();
}

export async function drop() {
    await utils.confirmAction();
    console.log('Dropping DB...');
    await utils.exec(`${SQL()} 'DROP OWNED BY CURRENT_USER CASCADE' ||
                     (${SQL()} 'DROP SCHEMA IF EXISTS public CASCADE' && ${SQL()} 'CREATE SCHEMA public')`);
}

export async function migrate() {
    await utils.confirmAction();
    console.log('Running migrations...');
    await utils.exec('cd core/lib/storage && diesel migration run');
}

export async function setup() {
    await basicSetup();
    await utils.spawn('cargo sqlx prepare --check -- --tests || cargo sqlx prepare -- --tests');
    process.chdir(process.env.ZKSYNC_HOME as string);
}

export async function basicSetup() {
    // force read env
    env.reload();

    process.chdir('core/lib/storage');
    if (process.env.DATABASE_URL == 'postgres://postgres@localhost/plasma') {
        console.log(`Using localhost database:`);
        console.log(`DATABASE_URL = ${process.env.DATABASE_URL}`);
    } else {
        // Remote database, we can't show the contents.
        console.log(`WARNING! Using prod db!`);
    }
    await utils.exec('diesel database setup');
    await utils.exec('diesel migration run');
    fs.unlinkSync('src/schema.rs.generated');
}

export async function updateToken(token: string, symbol: string) {
    console.log(`Setting token ${token} symbol to ${symbol}`);
    await utils.exec(`${SQL()} "UPDATE tokens SET symbol = '${symbol}' WHERE address = '${token}'"`);
}

export async function wait(tries: number = 4) {
    for (let i = 0; i < tries; i++) {
        const result = await utils.allowFail(utils.exec(`pg_isready -d "${process.env.DATABASE_URL}"`));
        if (result !== null) return; // null means failure
        await utils.sleep(5);
    }
    await utils.exec(`pg_isready -d "${process.env.DATABASE_URL}"`);
}

export const command = new Command('db')
    .description('database management')
    .addCommand(update.command)
    .addCommand(insert.command);

command.command('drop').description('drop the database').action(drop);
command.command('migrate').description('run migrations').action(migrate);
command
    .command('basic-setup')
    .description('initialize the database and perform migrations (without sqlx call)')
    .action(basicSetup);
command.command('setup').description('initialize the database and perform migrations').action(setup);
command.command('wait').description('wait for database to get ready for interaction').action(wait);
command.command('reset').description('reinitialize the database').action(reset);
