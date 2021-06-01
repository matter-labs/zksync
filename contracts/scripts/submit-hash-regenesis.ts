import { ArgumentParser } from 'argparse';
import { ethers } from 'ethers';
import * as fs from 'fs';
import * as path from 'path';
import { web3Provider } from './utils';
import { readProductionContracts } from '../src.ts/deploy';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));
const testContracts = readProductionContracts();

async function main() {
    const parser = new ArgumentParser({
        version: '0.0.1',
        addHelp: true,
        description: 'Submit signatures for regenesesis'
    });
    parser.addArgument('--masterPrivateKey');
    parser.addArgument('--contractAddress');
    parser.addArgument('--oldRootHash');
    parser.addArgument('--newRootHash');

    const args = parser.parseArgs(process.argv.slice(2));

    const provider = web3Provider();

    const wallet = args.masterPrivateKey
        ? new ethers.Wallet(args.masterPrivateKey).connect(provider)
        : ethers.Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);

    const contractAddress = args.contractAddress || process.env.MISC_REGENESIS_MULTISIG_ADDRESS;

    const contract = new ethers.Contract(contractAddress, testContracts.regenesisMultisig.abi, wallet);

    const oldRootHash = args.oldRootHash;
    const newRootHash = args.newRootHash;
    console.log('Submitting signatures...');
    console.log('Sender address: ', wallet.address);
    console.log('Contract address: ', contractAddress);
    console.log('OldHash: ', oldRootHash);
    console.log('NewHash: ', newRootHash);
    const tx = await contract.submitHash(oldRootHash, newRootHash, {
        gasLimit: 500000
    });

    await tx.wait();

    console.log('New hash submitted successfully');
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    })
    .finally(() => {});
