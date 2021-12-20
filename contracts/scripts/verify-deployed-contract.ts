import { Wallet } from 'ethers';
import { Deployer } from '../src.ts/deploy';
import * as fs from 'fs';
import * as path from 'path';
import { web3CustomProvider, web3Provider } from './utils';
import { Command } from 'commander';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

async function verify(localGeth: string, contractAddress: string, contract: string) {
    const mainProvider = web3Provider();
    const localProvider = web3CustomProvider(localGeth);

    const wallet = Wallet.fromMnemonic(
        process.env.MNEMONIC ? process.env.MNEMONIC : ethTestConfig.mnemonic,
        "m/44'/60'/0'/0/1"
    ).connect(localProvider);

    const gasPrice = await localProvider.getGasPrice();

    const governorAddress = wallet.address;
    console.log(`Deploying for governor: ${governorAddress}`);

    const deployer = new Deployer({ deployWallet: wallet, governorAddress, verbose: true });

    let localContractAddress;

    if (contract === 'RegenesisMultisig') {
        await deployer.deployRegenesisMultisig({ gasPrice });
        localContractAddress = deployer.addresses.RegenesisMultisig;
    }

    if (contract === 'AdditionalZkSync') {
        await deployer.deployAdditionalZkSync({ gasPrice });
        localContractAddress = deployer.addresses.AdditionalZkSync;
    }

    if (contract === 'ZkSync') {
        await deployer.deployZkSyncTarget({ gasPrice });
        localContractAddress = deployer.addresses.ZkSyncTarget;
    }

    if (contract === 'Verifier') {
        await deployer.deployVerifierTarget({ gasPrice });
        localContractAddress = deployer.addresses.VerifierTarget;
    }

    if (contract === 'Governance') {
        await deployer.deployGovernanceTarget({ gasPrice });
        localContractAddress = deployer.addresses.GovernanceTarget;
    }

    if (contract === 'TokenGovernance') {
        await deployer.deployTokenGovernance({ gasPrice });
        localContractAddress = deployer.addresses.TokenGovernance;
    }

    if (contract === 'ZkSyncNFTFactory') {
        await deployer.deployNFTFactory({ gasPrice });
        localContractAddress = deployer.addresses.NFTFactory;
    }

    if (contract === 'ForcedExit') {
        await deployer.deployForcedExit({ gasPrice });
        localContractAddress = deployer.addresses.ForcedExit;
    }
    const localBytecode = await localProvider.getCode(localContractAddress);
    const remoteBytecode = await mainProvider.getCode(contractAddress);

    console.log('Result of comparing bytecode', localBytecode === remoteBytecode);
}

async function main() {
    const program = new Command();

    program
        .version('0.1.0')
        .name('verify-contract')
        .description('Checking deployed Ethereum contract with locally deployed version');

    program
        .option('-g, --localGeth <localGeth>')
        .option('-c, --contract <contract>')
        .option('-a, --contractAddress <contractAddress>')
        .description('Checking deployed Ethereum contract with locally deployed version')
        .action(async (cmd: Command) => {
            await verify(cmd.localGeth, cmd.contractAddress, cmd.contract);
        });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
