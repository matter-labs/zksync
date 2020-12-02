// This script deploys a test "smart wallet" which supports EIP-1271 signatures.
// Owner account address is taken from the `$ZKSYNC_HOME/etc/test_config/eip1271.json`.
// Deployed contract address is updated in the same file.

import { ethers } from 'ethers';
import { readContractCode } from '../src.ts/deploy';
import { deployContract } from 'ethereum-waffle';
import * as fs from 'fs';
import * as path from 'path';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const EIP1271TestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eip1271.json`, { encoding: 'utf-8' }));
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

(async () => {
    try {
        if (!['test', 'localhost'].includes(process.env.ETH_NETWORK)) {
            console.error('This deploy script is only for localhost-test network');
            process.exit(1);
        }

        const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
        provider.pollingInterval = 10;

        const deployWallet = ethers.Wallet.fromMnemonic(ethTestConfig.test_mnemonic, "m/44'/60'/0'/0/0").connect(
            provider
        );
        const smartWallet = await deployContract(
            deployWallet,
            readContractCode('AccountMock'),
            [EIP1271TestConfig.owner_address],
            {
                gasLimit: 5000000
            }
        );

        const outConfig = {
            contract_address: smartWallet.address
        };
        const outConfigPath = path.join(process.env.ZKSYNC_HOME, 'etc/test_config/volatile/eip1271.json');
        fs.writeFileSync(outConfigPath, JSON.stringify(outConfig), { encoding: 'utf-8' });
        process.exit(0);
    } catch (err) {
        console.log(`Error: ${err}`);
        process.exit(1);
    }
})();
