import { Command } from 'commander';
import * as utils from './utils';

export async function restartLiquidityWatcher() {
    await utils.spawn('docker-compose restart dev-liquidity-token-watcher');
}

export const command = new Command('restart-liquidity-watcher')
    .description('Restart liquidity watcher container')
    .action(restartLiquidityWatcher);
