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
const wallet = Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);

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
    const erc20 = await deployContract(
        wallet,
        readContractCode(`dev-contracts/${token.implementation}`),
        [token.name, token.symbol, token.decimals],
        { gasLimit: 5000000 }
    );

    await erc20.mint(wallet.address, parseEther('3000000000'));
    for (let i = 0; i < 10; ++i) {
        const testWallet = Wallet.fromMnemonic(ethTestConfig.test_mnemonic as string, "m/44'/60'/0'/0/" + i).connect(
            provider
        );
        await erc20.mint(testWallet.address, parseEther('3000000000'));
    }
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
