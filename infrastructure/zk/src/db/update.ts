import { Command } from 'commander';
import * as utils from '../utils';

const SQL = () => `psql "${process.env.DATABASE_URL}" -c`;

export async function token(address: string, symbol: string) {
    console.log(`Setting token ${address} symbol to ${symbol}`);
    await utils.exec(`${SQL()} "UPDATE tokens SET symbol = '${symbol}' WHERE address = '${address}'"`);
}

export const command = new Command('update').description('update information in the database');

command.command('token <address> <symbol>').description('update token symbol').action(token);
