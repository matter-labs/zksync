import { ArgumentParser } from 'argparse';
import { Wallet } from 'ethers';
import { Deployer } from '../src.ts/deploy';
import { formatUnits, parseUnits } from 'ethers/lib/utils';
import * as fs from 'fs';
import * as path from 'path';
import { web3CustomProvider, web3Provider } from './utils';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

async function main() {
    const parser = new ArgumentParser({
        version: '0.1.0',
        addHelp: true,
        description: 'Deploy contracts and publish them on Etherscan'
    });

    parser.addArgument('--localGeth', { required: true, help: 'Localhost geth for deploying contract' });
    parser.addArgument('--contractAddress', { required: true, help: 'Already deployed contract onchain' });
    parser.addArgument('--contract', {
        required: false,
        help: 'Contract name: Governance, ZkSync, Verifier or all by default.'
    });

    const args = parser.parseArgs(process.argv.slice(2));

    const mainProvider = web3Provider();
    const localProvider = web3CustomProvider(args.localGeth);

    const wallet = Wallet.fromMnemonic(
        process.env.MNEMONIC ? process.env.MNEMONIC : ethTestConfig.mnemonic,
        "m/44'/60'/0'/0/1"
    ).connect(localProvider);

    const gasPrice = await localProvider.getGasPrice();
    console.log(`Using gas price: ${formatUnits(gasPrice, 'gwei')} gwei`);

    const governorAddress = wallet.address;
    console.log(`Deploying for governor: ${governorAddress}`);

    const deployer = new Deployer({ deployWallet: wallet, governorAddress, verbose: true });

    let localContractAddress;

    if (args.contract === 'RegenesisMultisig') {
        localContractAddress = await deployer.deployRegenesisMultisig({ gasPrice });
    }

    if (args.contract === 'AdditionalZkSync') {
        localContractAddress = await deployer.deployAdditionalZkSync({ gasPrice });
    }

    if (args.contract === 'ZkSync') {
        localContractAddress = await deployer.deployZkSyncTarget({ gasPrice });
    }

    if (args.contract === 'Verifier') {
        localContractAddress = await deployer.deployVerifierTarget({ gasPrice });
    }

    if (args.contract === 'Governance') {
        localContractAddress = await deployer.deployGovernanceTarget({ gasPrice });
    }

    if (args.contract === 'Proxies') {
        localContractAddress = await deployer.deployProxiesAndGatekeeper({ gasPrice });
    }

    if (args.contract === 'TokenGovernance') {
        localContractAddress = await deployer.deployTokenGovernance({ gasPrice });
    }

    if (args.contract === 'ZkSyncNFTFactory') {
        localContractAddress = await deployer.deployNFTFactory({ gasPrice });
    }

    if (args.contract === 'ForcedExit') {
        localContractAddress = await deployer.deployForcedExit({ gasPrice });
    }
    const localBytecode = await localProvider.getCode(localContractAddress);
    const remoteBytecode = await mainProvider.getCode(args.contractAddress);

    console.log('Result of comparing bytecode', localBytecode === remoteBytecode);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
