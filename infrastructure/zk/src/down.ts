import { Command } from 'commander';
import * as utils from './utils';

export async function down() {
    await utils.spawn('docker-compose stop tesseracts');
    await utils.spawn('docker-compose stop postgres geth dev-ticker dev-liquidity-token-watcher');
}

export const command = new Command('down').description('stop development containers').action(down);
