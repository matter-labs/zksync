import { ArgumentParser } from 'argparse';
import { Wallet } from 'ethers';
import { Deployer } from '../src.ts/deploy';
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

    const governorAddress = wallet.address;
    console.log(`Deploying for governor: ${governorAddress}`);

    const deployer = new Deployer({ deployWallet: wallet, governorAddress, verbose: true });

    let localContractAddress;

    if (args.contract === 'RegenesisMultisig') {
        await deployer.deployRegenesisMultisig({ gasPrice });
        localContractAddress = deployer.addresses.RegenesisMultisig;
    }

    if (args.contract === 'AdditionalZkSync') {
        await deployer.deployAdditionalZkSync({ gasPrice });
        localContractAddress = deployer.addresses.AdditionalZkSync;
    }

    if (args.contract === 'ZkSync') {
        await deployer.deployZkSyncTarget({ gasPrice });
        localContractAddress = deployer.addresses.ZkSync;
    }

    if (args.contract === 'Verifier') {
        await deployer.deployVerifierTarget({ gasPrice });
        localContractAddress = deployer.addresses.Verifier;
    }

    if (args.contract === 'Governance') {
        await deployer.deployGovernanceTarget({ gasPrice });
        localContractAddress = deployer.addresses.Governance;
    }

    if (args.contract === 'TokenGovernance') {
        await deployer.deployTokenGovernance({ gasPrice });
        localContractAddress = deployer.addresses.TokenGovernance;
    }

    if (args.contract === 'ZkSyncNFTFactory') {
        await deployer.deployNFTFactory({ gasPrice });
        localContractAddress = deployer.addresses.NFTFactory;
    }

    if (args.contract === 'ForcedExit') {
        await deployer.deployForcedExit({ gasPrice });
        localContractAddress = deployer.addresses.ForcedExit;
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
