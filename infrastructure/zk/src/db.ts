import { Command } from 'commander';
import * as utils from './utils';
import fs from 'fs';
import fetch from 'node-fetch';

const SQL = `psql "${process.env.DATABASE_URL}" -c`;

export async function drop() {
    console.log('Dropping DB...');
    await utils.exec(`${SQL} 'DROP OWNED BY CURRENT_USER CASCADE' ||
                     (${SQL} 'DROP SCHEMA IF EXISTS public CASCADE' && ${SQL} 'CREATE SCHEMA public')`);
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
    await utils.exec(`${SQL} "UPDATE tokens SET symbol = '${symbol}' WHERE address = '${token}'"`);
}

export async function insertToken(id: string, address: string, symbol: string, precison: string) {
    // force read env
    delete process.env.ZKSYNC_ENV;
    utils.loadEnv();
    await utils.exec(`${SQL} "INSERT INTO tokens VALUES (${id}, '${address}', '${symbol}', ${precison});"`);
    console.log('Successfully inserted token into the database');
}

export async function insertContract() {
    // force read env
    delete process.env.ZKSYNC_ENV;
    utils.loadEnv();
    const contractAddress = process.env.CONTRACT_ADDR;
    const govarnanceAddress = process.env.GOVERNANCE_ADDR;
    await utils.exec(`${SQL} "INSERT INTO server_config (contract_addr, gov_contract_addr)
					 VALUES ('${contractAddress}', '${govarnanceAddress}')
					 ON CONFLICT (id) DO UPDATE
					 SET (contract_addr, gov_contract_addr) = ('${contractAddress}', '${govarnanceAddress}')"`);
    console.log('Successfully inserted contract address into the database');
}

export async function insertEthData() {
    // force read env
    delete process.env.ZKSYNC_ENV;
    utils.loadEnv();

    const body = {
        jsonrpc: '2.0',
        method: 'eth_getTransactionCount',
        params: [process.env.OPERATOR_COMMIT_ETH_ADDRESS as string, 'pending'],
        id: 1
    };
    const reponse = await fetch(
        process.env.WEB3_URL as string,
        {
            method: 'post',
            body: JSON.stringify(body),
            headers: {
                Accept: 'application/json',
                'Content-type': 'application/json'
            },
        }
    );
    const nonce = parseInt((await reponse.json()).result);
    await utils.exec(`${SQL} "INSERT INTO eth_parameters (nonce, gas_price_limit, commit_ops, verify_ops, withdraw_ops)
                     VALUES ('${nonce}', '${process.env.ETH_GAS_PRICE_DEFAULT_LIMIT}', 0, 0, 0)
                     ON CONFLICT (id) DO UPDATE SET (commit_ops, verify_ops, withdraw_ops) = (0, 0, 0)"`);
}

export async function wait(tries: number = 5) {
    for (let i = 0; i < tries; i++) {
        try {
            await utils.exec(`pg_isready -d "${process.env.DATABASE_URL}"`);
            return;
        } catch (err) {}
        await utils.sleep(5);
    }
    await utils.exec(`pg_isready -d "${process.env.DATABASE_URL}"`);
}

// prettier-ignore
function buildCommand() {
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

    const updateCommand = new Command('update')
        .description('update information in the database');

    updateCommand
        .command('token <address> <symbol>')
        .description('update token symbol')
        .action(updateToken);

    const insertCommand = new Command('insert')
        .description('insert pre-defined data into the database');

    insertCommand
        .command('contract')
        .description('insert contract addresses')
        .action(insertContract);

    insertCommand
        .command('token <id> <address> <symbol> <precision>')
        .description('insert token information')
        .action(insertToken);

    insertCommand
        .command('eth-data')
        .description('insert info about Ethereum blockchain')
        .action(insertEthData);

    const command = new Command('db')
        .description('database management')
        .addCommand(dropCommand)
        .addCommand(migrateCommand)
        .addCommand(setupCommand)
        .addCommand(updateCommand)
        .addCommand(insertCommand)
        .addCommand(waitCommand);

    return command;
}

export const command = buildCommand();
