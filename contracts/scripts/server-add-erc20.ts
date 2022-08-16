import { Command } from 'commander';
import 'isomorphic-fetch';
import * as jwt from 'jsonwebtoken';
import { deployContract } from 'ethereum-waffle';
import { Wallet } from 'ethers';
import { readContractCode } from '../src.ts/deploy';
import { parseEther } from 'ethers/lib/utils';
import { web3Provider } from './utils';
import * as fs from 'fs';
import * as path from 'path';

type Token = {
    id: null;
    address: string;
    symbol: string;
    decimals: number;
};

async function addToken(token: Token) {
    console.log('Adding new ERC20 token to server: ', token.address);

    const tokenEndpoint = `${process.env.ADMIN_SERVER_API_URL}/tokens`;
    const authToken = jwt.sign(
        {
            sub: 'Authorization'
        },
        process.env.ADMIN_SERVER_SECRET_AUTH,
        { expiresIn: '1m' }
    );

    const response = await fetch(tokenEndpoint, {
        method: 'POST',
        headers: {
            Authorization: `Bearer ${authToken}`,
            'Content-Type': 'application/json'
        },
        body: JSON.stringify(token, null, 2)
    });

    console.log('-> REMEMBER TO ADD THE TOKEN TO THE GOVERNANCE CONTRACT NEXT');
    return await response.json();
}

async function deployTestnetToken(token: Token, name: String) {
    const DEFAULT_ERC20 = 'TestnetERC20Token';

    const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
    const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

    const provider = web3Provider();

    //So, instead, we will use an existing test account with RBTC balance
    let wallet = new Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow1_privK, 'hex'), provider);
    //const wallet = Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/137'/0'/0/1").connect(provider);
    const json_contract = readContractCode(`dev-contracts/${DEFAULT_ERC20}`);
    const token_arr = [name, token.symbol, token.decimals];
    const erc20 = await deployContract(wallet, json_contract, token_arr, { gasLimit: 5000000 });

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

    return erc20.address;
}

async function main() {
    const program = new Command();

    program.version('0.1.0').name('server-add-erc20').description('add erc20 token to the RIF Aggretation server');

    program
        .command('add')
        .option('-a, --address <address>')
        .option('-s, --symbol <symbol>')
        .option('-d, --decimals <decimals>')
        .description('Adds a new token with a given fields to the RIF Aggretation server')
        .action(async (cmd: Command) => {
            const token: Token = {
                id: null,
                address: cmd.address,
                symbol: cmd.symbol,
                decimals: Number(cmd.decimals)
            };

            console.log(JSON.stringify(await addToken(token), null, 2));
        });

    program
        .command('add-and-mint')
        .option('-sn, --sname <s_name>')
        .option('-s, --symbol <symbol>')
        .option('-d, --decimals <decimals>')
        .option('-e, --env <env>')
        .action(async ({ decimals, s_name, address, symbol, env }: Command) => {
            const token: Token = {
                id: null,
                address: address,
                symbol: symbol,
                decimals: Number(decimals)
            };

            if (env === 'local') {
                const address = await deployTestnetToken(token, s_name + '');
                console.log('TOKEN: ' + address);

                token.address = address;
            }

            const response = await addToken(token);
            console.log(JSON.stringify(await response, null, 2));
            console.log('-> REMEMBER TO ADD THE TOKEN TO THE GOVERNANCE CONTRACT NEXT');
        });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
