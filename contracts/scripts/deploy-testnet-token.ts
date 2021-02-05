import { deployContract } from 'ethereum-waffle';
import { Wallet } from 'ethers';
import { readContractCode } from '../src.ts/deploy';
import { encodeConstructorArgs, publishSourceCodeToEtherscan } from '../src.ts/publish-utils';
import * as fs from 'fs';
import * as path from 'path';
import { ArgumentParser } from 'argparse';
import { web3Provider } from './utils';

const mainnetTokens = require(`${process.env.ZKSYNC_HOME}/etc/tokens/mainnet`);

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

async function main() {
    const parser = new ArgumentParser({
        version: '0.1.0',
        addHelp: true,
        description: 'Deploy contracts and publish them on Etherscan/Tesseracts'
    });
    parser.addArgument('--publish', {
        required: false,
        action: 'storeTrue',
        help: 'Only publish code for deployed tokens'
    });
    parser.addArgument('--deployerPrivateKey', { required: false, help: 'Wallet used to deploy contracts' });
    const args = parser.parseArgs(process.argv.slice(2));

    const provider = web3Provider();
    const wallet = args.deployerPrivateKey
        ? new Wallet(args.deployerPrivateKey, provider)
        : Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);

    const contractCode = readContractCode('TestnetERC20Token');

    if (process.env.CHAIN_ETH_NETWORK === 'mainnet') {
        throw new Error('Test ERC20 tokens should not be deployed to mainnet');
    }

    if (args.publish) {
        console.log('Publishing source code');
        let verifiedOnce = false;
        const networkTokens = require(`${process.env.ZKSYNC_HOME}/etc/tokens/${process.env.ETH_NETWORK}`);
        for (const token of networkTokens) {
            if (verifiedOnce) {
                break;
            }
            try {
                console.log(`Publishing code for : ${token.symbol}, ${token.address}`);
                const constructorArgs = [
                    `${token.name} (${process.env.CHAIN_ETH_NETWORK})`,
                    token.symbol,
                    token.decimals
                ];
                const rawArgs = encodeConstructorArgs(contractCode, constructorArgs);
                await publishSourceCodeToEtherscan(token.address, 'TestnetERC20Token', rawArgs, 'contracts/test');
                verifiedOnce = true;
            } catch (e) {
                console.log('Error failed to verified code:', e);
            }
        }
        return;
    }

    const result = [];

    for (const token of mainnetTokens) {
        const constructorArgs = [`${token.name} (${process.env.CHAIN_ETH_NETWORK})`, token.symbol, token.decimals];

        console.log(`Deploying testnet ERC20: ${constructorArgs.toString()}`);
        const erc20 = await deployContract(wallet, contractCode, constructorArgs, { gasLimit: 800000 });

        const testnetToken = token;
        testnetToken.address = erc20.address;
        result.push(testnetToken);
    }

    fs.writeFileSync(
        `${process.env.ZKSYNC_HOME}/etc/tokens/${process.env.CHAIN_ETH_NETWORK}.json`,
        JSON.stringify(result, null, 2)
    );
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
