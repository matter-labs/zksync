import { Command } from 'commander';
import * as utils from './utils';
import { Wallet } from 'ethers';

async function revertReason(txHash: string, web3url?: string) {
    await utils.spawn(`cd contracts && npx ts-node revert-reason.ts ${txHash} ${web3url || ''}`);
}

async function explorer() {
    await utils.spawn('yarn --cwd infrastructure/explorer serve');
}

async function exitProof(...args: string[]) {
    await utils.spawn(`cargo run --example generate_exit_proof --release -- ${args.join(' ')}`);
}

async function testAccounts() {
    const NUM_TEST_WALLETS = 10;
    const baseWalletPath = "m/44'/60'/0'/0/";
    const walletKeys = [];
    for (let i = 0; i < NUM_TEST_WALLETS; ++i) {
        const ethWallet = Wallet.fromMnemonic(process.env.TEST_MNEMONIC as string, baseWalletPath + i);
        walletKeys.push({
            address: ethWallet.address,
            privateKey: ethWallet.privateKey
        });
    }
    console.log(JSON.stringify(walletKeys, null, 4));
}

async function verifyKeys() {}
async function loadtest() {}


export const command = new Command('run')
    .description('run miscellaneous applications')

command
    .command('test-accounts')
    .description('print ethereum test accounts')
    .action(testAccounts);

command
    .command('explorer')
    .description('run zksync explorer locally')
    .action(explorer)

command
    .command('revert-reason <tx_hash> [web3_url]')
    .description('get the revert reason for ethereum transaction')
    .action(revertReason);

command
    .command('exit-proof')
    .option('--account <id>')
    .option('--token <id>')
    .option('--help')
    .description('generate exit proof')
    .action(async (cmd: Command) => {
        if (!cmd.account || !cmd.token) {
            await exitProof('--help');
        } else {
            await exitProof('--account_id', cmd.account, '--token', cmd.token);
        }
    });
