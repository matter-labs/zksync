import { ArgumentParser } from 'argparse';
import * as fs from 'fs';
import * as path from 'path';
import { Wallet } from 'ethers';
import { Deployer } from '../src.ts/deploy';

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

async function main() {
    const parser = new ArgumentParser({
        version: '0.1.0',
        addHelp: true
    });
    parser.addArgument('--deployerPrivateKey', { required: false, help: 'Wallet used to deploy contracts' });
    const args = parser.parseArgs(process.argv.slice(2));

    const wallet = args.deployerPrivateKey
        ? new Wallet(args.deployerPrivateKey, provider)
        : Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);

    const deployer = new Deployer({ deployWallet: wallet });
    const upgradeGatekeeper = deployer.upgradeGatekeeperContract(wallet);
    const tx = await upgradeGatekeeper.finishUpgrade(['0x', '0x', '0x']);
    console.log(tx);
}

main();
