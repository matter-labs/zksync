#!/usr/bin/env node

import { Command } from 'commander';
import * as commands from './commands';
import { loadConfig } from './config';
import type { Network } from './types';

function print(object: any) {
    console.log(JSON.stringify(object, null, 4));
}

async function main() {
    const config = loadConfig();
    const program = new Command();

    const handler = async (
        operation: 'transfer' | 'deposit',
        fast: boolean,
        json?: string,
        amount?: string,
        token?: string,
        recipient?: string
    ) => {
        if (json && (amount || token || recipient)) {
            throw new Error('--json option and positional arguments are mutually exclusive');
        }
        if (!config.defaultWallet && !json) {
            throw new Error('sender is not provided');
        }
        if (operation == 'deposit') {
            recipient = recipient || config.defaultWallet || '';
        }
        // prettier-ignore
        const txDetails = json ? JSON.parse(json) : {
            privkey: config.wallets[config.defaultWallet as any],
            to: recipient,
            amount,
            token
        };
        const hash = await commands.submitTx(operation, txDetails, fast, program.network);
        print(fast ? hash : await commands.txInfo(hash, program.network));
    };

    program.version('0.1.0').name('zcli').option('-n, --network <network>', 'select network', config.network);

    program
        .command('account [address]')
        .description('view account info')
        .action(async (address?: string) => {
            if (!address && !config.defaultWallet) {
                throw new Error('no address provided');
            }
            address = address || config.defaultWallet || '';
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
        .option('--fast', 'do not wait for transaction commitment')
        .action(async (amount, token, recipient, cmd) => {
            await handler('transfer', cmd.fast, cmd.json, amount, token, recipient);
        });

    program
        .command('deposit [amount] [token] [recipient]')
        .description('make a deposit')
        .option('--json <string>', 'supply deposit info as json string')
        .option('--fast', 'do not wait for transaction commitment')
        .action(async (amount, token, recipient, cmd) => {
            await handler('deposit', cmd.fast, cmd.json, amount, token, recipient);
        });

    program
        .command('await <type> <tx_hash>')
        .description('await for transaction commitment/verification')
        .option('-t, --timeout <sec>', 'set a timeout', '60')
        .action(async (type: string, tx_hash: string, cmd: Command) => {
            if (type !== 'verify' && type !== 'commit') {
                throw new Error('can only "await commit" or "await verify"');
            }
            const timeout = Number.parseFloat(cmd.timeout) * 1000;
            // prettier-ignore
            print(await Promise.race([
                commands.txInfo(tx_hash, program.network, type.toUpperCase() as any),
                new Promise((resolve) => setTimeout(resolve, timeout, null))
            ]));
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
            console.warn('[WARNING]: private keys are stored unencrypted');
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

main()
    .then(() => process.exit(0))
    .catch((err: Error) => {
        console.error('Error:', err.message);
        process.exit(1);
    });
