#!/usr/bin/env node

import { Command } from 'commander';
import * as commands from './commands';
import { loadConfig, saveConfig } from './config';

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
        .action((network?: commands.Network) => {
            if (network) {
                if (commands.ALL_NETWORKS.includes(network)) {
                    config.network = network;
                    saveConfig(config);
                } else {
                    throw Error('invalid network name');
                }
            }
            print(config.network);
        });

    program.addCommand(networks);

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err: Error) => {
        console.error(err);
        process.exit(1);
    });

