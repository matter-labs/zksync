import { Command } from 'commander';
import { BigNumber, Wallet, ethers } from 'ethers';
import { Deployer } from '../src.ts/deploy';
import * as fs from 'fs';
import * as path from 'path';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));
const deployer = new Deployer({ deployWallet: ethers.Wallet.createRandom() });
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const governorWallet = Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);

async function governanceAddToken(address: string) {
    console.log('Adding new ERC20 token to network: ', address);

    const tx = await deployer
        .governanceContract(governorWallet)
        .addToken(address, { gasLimit: BigNumber.from('1000000') });
    console.log('tx hash: ', tx.hash);
    const receipt = await tx.wait();

    if (receipt.status) {
        console.log('tx success');
    } else {
        throw new Error(`failed add token to the governance`);
    }
}

async function main() {
    const program = new Command();

    program.version('0.1.0').name('governance-add-erc20').description('add testnet erc20 token to the governance');

    program
        .command('add')
        .option('-a, --address <address>')
        .description('Adds a new token with a given address')
        .action(async (address: string) => {
            await governanceAddToken(address);
        });

    program
        .command('add-multi <tokens_json>')
        .description('Adds a multiple tokens given in JSON format')
        .action(async (tokens_json: string) => {
            const tokens: Array<string> = JSON.parse(tokens_json);

            for (const token of tokens) {
                await governanceAddToken(token);
            }
        });

    program.parse(process.argv);
}

main();
