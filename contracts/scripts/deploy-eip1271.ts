/**
 * Optimized Deployment Script for EIP-1271 Smart Wallet
 * Implements better error handling and path resolution
 */

import { ethers } from 'ethers';
import { readContractCode } from '../src.ts/deploy';
import { deployContract } from 'ethereum-waffle';
import * as fs from 'fs';
import * as path from 'path';
import { web3Provider } from './utils';

// Guard: Ensure required environment variables exist
if (!process.env.ZKSYNC_HOME) {
    throw new Error('ZKSYNC_HOME environment variable is not set');
}

const ZKSYNC_HOME = process.env.ZKSYNC_HOME;
const TEST_CONFIG_BASE = path.join(ZKSYNC_HOME, 'etc/test_config/constant');
const VOLATILE_CONFIG_PATH = path.join(ZKSYNC_HOME, 'etc/test_config/volatile/eip1271.json');

/**
 * Loads configuration files with error checking
 */
function loadConfig(fileName: string) {
    const filePath = path.join(TEST_CONFIG_BASE, fileName);
    if (!fs.existsSync(filePath)) {
        throw new Error(`Config file not found: ${filePath}`);
    }
    return JSON.parse(fs.readFileSync(filePath, 'utf-8'));
}

async function main() {
    const network = process.env.CHAIN_ETH_NETWORK;
    if (!['test', 'localhost'].includes(network || '')) {
        throw new Error(`Invalid network: ${network}. Deploy script is restricted to localhost/test.`);
    }

    const EIP1271TestConfig = loadConfig('eip1271.json');
    const ethTestConfig = loadConfig('eth.json');

    const provider = web3Provider();
    provider.pollingInterval = 10;

    // Connect deployer wallet using mnemonic
    const deployWallet = ethers.Wallet.fromMnemonic(
        ethTestConfig.test_mnemonic, 
        "m/44'/60'/0'/0/0"
    ).connect(provider);

    console.log(`Deploying AccountMock from: ${deployWallet.address}...`);

    // Contract Deployment
    const accountMockArtifact = readContractCode('dev-contracts/AccountMock');
    const smartWallet = await deployContract(
        deployWallet,
        accountMockArtifact,
        [EIP1271TestConfig.owner_address],
        { gasLimit: 5000000 }
    );

    console.log(`Success! Smart Wallet deployed at: ${smartWallet.address}`);

    // Update Volatile Config
    const outConfig = {
        contract_address: smartWallet.address
    };

    // Ensure directory exists before writing
    fs.mkdirSync(path.dirname(VOLATILE_CONFIG_PATH), { recursive: true });
    fs.writeFileSync(VOLATILE_CONFIG_PATH, JSON.stringify(outConfig, null, 2), 'utf-8');
}

main()
    .then(() => {
        console.log('Deployment cycle completed successfully.');
        process.exit(0);
    })
    .catch((err) => {
        console.error('Deployment Failed:');
        console.error(err instanceof Error ? err.message : err);
        process.exit(1);
    });
