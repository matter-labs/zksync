import { Command } from 'commander';
import * as utils from './utils';
import fs from 'fs';

const SQL = `psql "${process.env.DATABASE_URL}" -c`;

export async function drop() {
	console.log("Dropping DB...");
	await utils.exec(`${SQL} 'DROP OWNED BY CURRENT_USER CASCADE' || 
                     (${SQL} 'DROP SCHEMA IF EXISTS public CASCADE' && ${SQL} 'CREATE SCHEMA public')`);
}

export async function migrate() {
    console.log("Running migrations...");
    await utils.exec("cd core/storage && diesel migration run");
}

export async function setup() {
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
	await utils.exec(`${SQL} "INSERT INTO tokens VALUES (${id}, '${address}', '${symbol}', ${precison});"`)
	console.log("Successfully inserted token into the database");
}

export async function insertContract(id: string, address: string, symbol: string, precison: string) {
	await utils.exec(`${SQL} "INSERT INTO server_config (contract_addr, gov_contract_addr) \
							 VALUES ('$CONTRACT_ADDR', '$GOVERNANCE_ADDR') \
							 ON CONFLICT (id) DO UPDATE  \
							 SET (contract_addr, gov_contract_addr) = ('$CONTRACT_ADDR', '$GOVERNANCE_ADDR')"`);
	console.log("successfully inserted contract address into the database");
}

const dropCommand = new Command('drop')
    .description('drop the database')
    .action(drop);

const migrateCommand = new Command('migrate')
    .description('run migrations')
    .action(migrate);

const setupCommand = new Command('setup')
    .description('initialize the database and perform migrations')
    .action(async () => {
		utils.loadEnv();
		await setup();
	});

const updateCommand = new Command('update')
    .description('update information in the database');
updateCommand
	.command('token <address> <symbol>')
	.description('update token symbol')
    .action(updateToken);

export const command = new Command('db')
    .description('database management')
    .addCommand(dropCommand)
    .addCommand(migrateCommand)
    .addCommand(setupCommand)
    .addCommand(updateCommand)

