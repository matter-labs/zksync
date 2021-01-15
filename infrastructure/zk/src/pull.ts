import { Command } from 'commander';
import * as utils from './utils';

export async function pull() {
    await utils.spawn('docker-compose pull');
}

export const command = new Command('pull').description('start development containers').action(pull);
