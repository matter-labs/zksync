import { ethers, Wallet } from 'ethers';
import { Deployer, readContractCode, readProductionContracts } from '../src.ts/deploy';
import { deployContract } from 'ethereum-waffle';
import { ArgumentParser } from 'argparse';

import * as fs from 'fs';
import * as path from 'path';
import { web3Provider } from './utils';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

async function main() {
    const parser = new ArgumentParser({
        version: '0.1.0',
        addHelp: true,
        description: 'Deploy testkit contracts'
    });
    parser.addArgument('--prodContracts', {
        required: false,
        help: 'deploy production contracts',
        action: 'storeTrue'
    });
    parser.addArgument('--genesisRoot', { required: true, help: 'genesis root' });
    const args = parser.parseArgs(process.argv.slice(2));
    process.env.CONTRACTS_GENESIS_ROOT = args.genesisRoot;

    if (process.env.CHAIN_ETH_NETWORK !== 'test') {
        console.error('This deploy script is only for localhost-test network');
        process.exit(1);
    }

    const provider = web3Provider();
    provider.pollingInterval = 10;

    const deployWallet = ethers.Wallet.fromMnemonic(ethTestConfig.test_mnemonic, "m/44'/60'/0'/0/0").connect(provider);
    // todo: should be decided when building
    const contracts = readProductionContracts();
    const deployer = new Deployer({ deployWallet, contracts, verbose: true });
    await deployer.deployAll();
    const governance = deployer.governanceContract(deployWallet);
    await (await governance.setValidator(deployWallet.address, true)).wait();

    const erc20 = await deployContract(
        deployWallet,
        readContractCode('dev-contracts/TestnetERC20Token'),
        ['Matter Labs Trial Token', 'MLTT', 18],
        { gasLimit: 5000000 }
    );
    console.log(`CONTRACTS_TEST_ERC20=${erc20.address}`);
    await (await governance.addToken(erc20.address)).wait();
    if ((await governance.tokenIds(erc20.address)) !== 1) {
        console.error('Problem with testkit deployment, TEST_ERC20 token should have id 1');
        process.exit(1);
    }

    for (let i = 0; i < 10; ++i) {
        const testWallet = Wallet.fromMnemonic(ethTestConfig.test_mnemonic, "m/44'/60'/0'/0/" + i).connect(provider);
        await (await erc20.mint(testWallet.address, '0x4B3B4CA85A86C47A098A224000000000')).wait();
    }
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
