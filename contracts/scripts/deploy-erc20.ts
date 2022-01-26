import { Command } from 'commander';
import { deployContract } from 'ethereum-waffle';
import { Wallet } from 'ethers';
import { readContractCode } from '../src.ts/deploy';
import { parseEther } from 'ethers/lib/utils';
import { web3Provider } from './utils';
import * as fs from 'fs';
import * as path from 'path';

const DEFAULT_ERC20 = 'TestnetERC20Token';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

const provider = web3Provider();

type Token = {
    address: string | null;
    name: string;
    symbol: string;
    decimals: number;
};

type TokenDescription = Token & {
    implementation?: string;
};

async function deployToken(token: TokenDescription): Promise<Token> {
    token.implementation = token.implementation || DEFAULT_ERC20;

    //So, instead, we will use an existing test account with RBTC balance
    let wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow1_privK, 'hex'), provider);
    //const wallet = Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/137'/0'/0/1").connect(provider);
    const erc20 = await deployContract(
        wallet,
        readContractCode(`dev-contracts/${token.implementation}`),
        [token.name, token.symbol, token.decimals],
        { gasLimit: 5000000 }
    );

    //These erc20 tokens are deployed and allocated to some accounts just for testing.
    //These deployments initially lead to a "nonce too high error on RSKJ" during zk init.
    //So we added "waits" for contract deployment TX to be mined. The following WILL BLOCK execution until no. of confirmations is met
    //use this together with tx.wait() keep the accountSlots / nonce mismatch within 5
    // We do not use the timeout paramater. Just block execution until token is deployed.
    await provider.waitForTransaction(erc20.deployTransaction.hash, 1);

    // now we premine to allocate token balances to some accounts

    await erc20.mint(wallet.address, parseEther('3000000000'));
    wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow_privK, 'hex'), provider);
    await erc20.mint(wallet.address, parseEther('3000000000'));
    wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow2_privK, 'hex'), provider);

    // occasionally wait for 1 confirmation to keep nonce increase within 5
    let tx = await erc20.mint(wallet.address, parseEther('3000000000'));
    await tx.wait(1);

    wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow3_privK, 'hex'), provider);
    await erc20.mint(wallet.address, parseEther('3000000000'));
    wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow4_privK, 'hex'), provider);
    await erc20.mint(wallet.address, parseEther('3000000000'));
    wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow5_privK, 'hex'), provider);
    //wait
    tx = await erc20.mint(wallet.address, parseEther('3000000000'));
    await tx.wait(1);

    wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow7_privK, 'hex'), provider);
    await erc20.mint(wallet.address, parseEther('3000000000'));
    wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow8_privK, 'hex'), provider);
    await erc20.mint(wallet.address, parseEther('3000000000'));
    wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow9_privK, 'hex'), provider);
    //wait
    tx = await erc20.mint(wallet.address, parseEther('3000000000'));
    await tx.wait(1);

    token.address = erc20.address;

    // Remove the unneeded field
    if (token.implementation) {
        delete token.implementation;
    }

    return token;
}

async function main() {
    const program = new Command();

    program.version('0.1.0').name('deploy-erc20').description('deploy testnet erc20 token');

    program
        .command('add')
        .option('-n, --token-name <token_name>')
        .option('-s, --symbol <symbol>')
        .option('-d, --decimals <decimals>')
        .option('-i --implementation <implementation>')
        .description('Adds a new token with a given fields')
        .action(async (cmd: Command) => {
            const token: TokenDescription = {
                address: null,
                name: cmd.token_name,
                symbol: cmd.symbol,
                decimals: cmd.decimals,
                implementation: cmd.implementation
            };
            console.log(JSON.stringify(await deployToken(token), null, 2));
        });

    program
        .command('add-multi <tokens_json>')
        .description('Adds a multiple tokens given in JSON format')
        .action(async (tokens_json: string) => {
            const tokens: Array<TokenDescription> = JSON.parse(tokens_json);
            const result = [];

            for (const token of tokens) {
                result.push(await deployToken(token));
            }

            console.log(JSON.stringify(result, null, 2));
        });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
