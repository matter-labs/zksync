// This script deploys the contracts required both for production and
// for testing of the contracts required for the `withdrawal-helpers` library

import { ethers } from 'ethers';
import { deployContract } from 'ethereum-waffle';
import * as fs from 'fs';
import * as path from 'path';

import { readContractCode } from '../src.ts/deploy';
import { web3Provider } from './utils';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

async function main() {
    try {
        if (!['test', 'localhost'].includes(process.env.CHAIN_ETH_NETWORK)) {
            console.error('This deploy script is only for localhost-test network');
            process.exit(1);
        }

        const provider = web3Provider();
        provider.pollingInterval = 10;

        const deployWallet = ethers.Wallet.fromMnemonic(ethTestConfig.test_mnemonic, "m/44'/60'/0'/0/0").connect(
            provider
        );
        const multicallContract = await deployContract(deployWallet, readContractCode('dev-contracts/Multicall'), [], {
            gasLimit: 5000000
        });
        const revertReceiveAccount = await deployContract(
            deployWallet,
            readContractCode('dev-contracts/RevertReceiveAccount'),
            [],
            {
                gasLimit: 5000000
            }
        );

        const outConfig = {
            multicall_address: multicallContract.address,
            revert_receive_address: revertReceiveAccount.address
        };
        const outConfigPath = path.join(process.env.ZKSYNC_HOME, 'etc/test_config/volatile/withdrawal-helpers.json');
        fs.writeFileSync(outConfigPath, JSON.stringify(outConfig), { encoding: 'utf-8' });
        process.exit(0);
    } catch (err) {
        console.log(`Error: ${err}`);
        process.exit(1);
    }
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
