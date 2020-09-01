#!/usr/bin/env node

import { Command } from 'commander';
import { accountInfo, txInfo } from './commands';
import { loadConfig, saveConfig } from './config';

async function main() {

    const config = loadConfig();
    const program = new Command();
    
    program
        .version("0.1.0")
        .name("zcli")
        .option("-n, --network <network>", "select network", config.network || "rinkeby")

    program
        .command("account <address>")
        .action(async (address: string) => {
            const info = await accountInfo(address, program.network)
            console.log(JSON.stringify(info, null, 4));
        });

    program
        .command("transaction <tx_hash>")
        .action(async (tx_hash: string) => {
            const info = await txInfo(tx_hash, program.network);
            console.log(JSON.stringify(info, null, 4));
        });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err: Error) => {
        console.error(err);
        process.exit(1);
    });

