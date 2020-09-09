#!/usr/bin/env node

import { Command } from 'commander';
import * as commands from './commands';
import { loadConfig } from './config';
import type { Network } from './common';

function print(object: any) {
    console.log(JSON.stringify(object, null, 4));
}

async function main() {
    const config = loadConfig();
    const program = new Command();

    program
        .version('0.1.0')
        .name('zcli')
        .option('-n, --network <network>', 'select network', config.network);

    program
        .command('account <address>')
        .description('view account info')
        .action(async (address: string) => {
            print(await commands.accountInfo(address, program.network));
        });

    program
        .command('transaction <tx_hash>')
        .description('view transaction info')
        .action(async (tx_hash: string) => {
            print(await commands.txInfo(tx_hash, program.network));
        });

    program
        .command('transfer [amount] [token] [recipient]')
        .description('make a transfer')
        .option('--json <string>', 'supply transfer info as json string')
        .action(async (amount: string, token: string, recipient: string, cmd: Command) => {
            if (!config.defaultWallet && !cmd.json) {
                throw new Error('default wallet is not set');
            }
            // prettier-ignore
            const transferInfo = cmd.json ? JSON.parse(cmd.json) : {
                // @ts-ignore
                privkey: config.wallets[config.defaultWallet],
                to: recipient,
                amount,
                token
            };
            const hash = await commands.transfer(transferInfo, program.network);
            print(await commands.txInfo(hash, program.network));
        });

    program
        .command('deposit [amount] [token] [recipient]')
        .description('make a deposit')
        .option('--json <string>', 'supply deposit info as json string')
        .action(async (amount: string, token: string, recipient: string, cmd: Command) => {
            if (!config.defaultWallet && !cmd.json) {
                throw new Error('default wallet is not set');
            }
            // prettier-ignore
            const depositInfo = cmd.json ? JSON.parse(cmd.json) : {
                // @ts-ignore
                privkey: config.wallets[config.defaultWallet],
                to: recipient,
                amount,
                token
            };
            const hash = await commands.deposit(depositInfo, program.network);
            print(await commands.txInfo(hash, program.network));
        });

    const networks = new Command('networks');

    networks
        .description('view configured networks')
        .action(async () => {
            print(await commands.availableNetworks());
        })
        .command('default [network]')
        .description('print or set default network')
        .action((network?: Network) => {
            print(commands.defaultNetwork(config, network));
        });

    program.addCommand(networks);

    const wallets = new Command('wallets');

    wallets
        .description('view saved wallets')
        .action(() => {
            print(commands.listWallets(config));
        })
        .command('add [private_key]')
        .description('create or import a wallet')
        .action((privkey?: string) => {
            print(commands.addWallet(config, privkey));
        });

    wallets
        .command('default [address]')
        .description('print or set default wallet')
        .action((address?: string) => {
            print(commands.defaultWallet(config, address));
        });

    wallets
        .command('delete <address>')
        .description('delete a wallet')
        .action((address: string) => {
            commands.removeWallet(config, address);
            print(commands.listWallets(config));
        });

    program.addCommand(wallets);

    await program.parseAsync(process.argv);
}

main().catch((err: Error) => {
    console.error('Error:', err.message);
    process.exit(1);
});
