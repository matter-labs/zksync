#!/usr/bin/env node

import { Command } from 'commander';
import * as commands from './commands';
import { loadConfig, saveConfig } from './config';
import { Network, Wallet, ALL_NETWORKS } from './common';

function print(object: any) {
    console.log(JSON.stringify(object, null, 4));
};

async function main() {

    const config = loadConfig();
    const program = new Command();

    program
        .version("0.1.0")
        .name("zcli")
        .option("-n, --network <network>", "select network", config.network)

    program
        .command("account <address>")
        .description("view account info")
        .action(async (address: string) => {
            print(await commands.accountInfo(address, program.network));
        });

    program
        .command("transaction <tx_hash>")
        .description("view transaction info")
        .action(async (tx_hash: string) => {
            print(await commands.txInfo(tx_hash, program.network));
        });

    const networks = new Command('networks');

    networks
        .description('view configured networks')
        .action(async () => {
            print(await commands.availableNetworks());
        });

    networks
        .command('default [network]')
        .description('print or set default network')
        .action((network?: Network) => {
            if (network) {
                if (ALL_NETWORKS.includes(network)) {
                    config.network = network;
                    saveConfig(config);
                } else {
                    throw Error('invalid network name');
                }
            }
            print(config.network);
        });

    program.addCommand(networks);

    const wallets = new Command('wallets');

    wallets
        .description('view saved wallets')
        .action(() => {
            print(commands.listWallets(config));
        });

    wallets
        .command('add [private_key]')
        .description('create or import a wallet')
        .action((privkey?: string) => {
            print(commands.addWallet(config, privkey));
        });

    wallets
        .command('default [address]')
        .description('print or set default wallet')
        .action((address?: string) => {
            if (address) {
                const addresses = config.wallets
                    .map((w: Wallet) => w.address.toLowerCase());
                if (addresses.includes(address.toLowerCase())) {
                    config.defaultWallet = address;
                    saveConfig(config);
                } else {
                    throw Error('address is not present');
                }
            }
            print(config.defaultWallet);
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
        console.error(err);
        process.exit(1);
    });

