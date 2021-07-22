import { ethers, Wallet } from 'ethers';
import * as fs from 'fs';
import * as path from 'path';
import { Deployer } from '../src.ts/deploy';

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));
const wallet = Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);
const deployer = new Deployer({ deployWallet: wallet });

async function main() {
    const governanceContract = deployer.governanceContract(wallet);
    console.log('total tokens', await governanceContract.totalTokens());
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
