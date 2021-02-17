import { Command } from 'commander';
import * as utils from './utils';
import * as env from './env';
import fs from 'fs';
import * as db from './db/db';

import { ethers } from 'ethers';

export async function server() {
    let child = utils.background('cargo run --bin zksync_server --release');

    // delegate processing of pressing `Ctrl + C`
    process.on('SIGINT', () => {
        child.kill('SIGINT');
    });

    // By the time this function is run the server is most likely not be running yet
    // However, it does not matter, since the only thing the function does is depositing
    // to the forced exit sender account, and server should be capable of recognizing
    // priority operaitons that happened before it was booted
    await prepareForcedExitRequestAccount();
}

export async function genesis() {
    await db.reset();
    await utils.confirmAction();
    await utils.spawn('cargo run --bin zksync_server --release -- --genesis | tee genesis.log');
    const genesisRoot = fs.readFileSync('genesis.log').toString().trim();
    const date = new Date();
    const [year, month, day, hour, minute, second] = [
        date.getFullYear(),
        date.getMonth(),
        date.getDate(),
        date.getHours(),
        date.getMinutes(),
        date.getSeconds()
    ];
    const label = `${process.env.ZKSYNC_ENV}-Genesis_gen-${year}-${month}-${day}-${hour}${minute}${second}`;
    fs.mkdirSync(`logs/${label}`, { recursive: true });
    fs.copyFileSync('genesis.log', `logs/${label}/genesis.log`);
    env.modify('CONTRACTS_GENESIS_ROOT', genesisRoot);
    env.modify_contracts_toml('CONTRACTS_GENESIS_ROOT', genesisRoot);
}

// This functions deposits funds onto the forced exit sender account
// This is needed to make sure that it has the account id
async function prepareForcedExitRequestAccount() {
    console.log('Depositing to the forced exit sender account');
    const forcedExitAccount = process.env.FORCED_EXIT_REQUESTS_SENDER_ACCOUNT_ADDRESS as string;

    // This is the private key of the first test account
    const ethProvider = new ethers.providers.JsonRpcProvider('http://localhost:8545');
    const ethRichWallet = new ethers.Wallet('0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110');

    const mainZkSyncContract = new ethers.Contract(
        process.env.CONTRACTS_CONTRACT_ADDR as string,
        await utils.readZkSyncAbi(),
        ethRichWallet.connect(ethProvider)
    );
    const gasPrice = await ethProvider.getGasPrice();

    const ethTransaction = (await mainZkSyncContract.depositETH(forcedExitAccount, {
        // The amount to deposit does not really matter
        value: ethers.utils.parseEther('1.0'),
        gasLimit: ethers.BigNumber.from('200000'),
        gasPrice
    })) as ethers.ContractTransaction;

    await ethTransaction.wait();

    console.log('Deposit to the forced exit sender account has been successfully completed');
}

export const command = new Command('server')
    .description('start zksync server')
    .option('--genesis', 'generate genesis data via server')
    .action(async (cmd: Command) => {
        if (cmd.genesis) {
            await genesis();
        } else {
            await server();
        }
    });
