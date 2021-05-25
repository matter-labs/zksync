import { ArgumentParser } from 'argparse';
import { deployContract } from 'ethereum-waffle';
import { ethers } from 'ethers';
import * as fs from 'fs';
import * as path from 'path';
import { web3Provider, storedBlockInfoParam } from './utils';
import { readProductionContracts } from '../src.ts/deploy';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));
const testContracts = readProductionContracts();

async function main() {
    const parser = new ArgumentParser({
        version: '0.0.1',
        addHelp: true,
        description: 'Contract upgrade'
    });
    parser.addArgument('--masterPrivateKey');
    parser.addArgument('--upgradeGatekeeperAddress');
    parser.addArgument('--lastBlockInfo');
    parser.addArgument('--finish');
    parser.addArgument('--startPreparation');
    const args = parser.parseArgs(process.argv.slice(2));

    const provider = web3Provider();
    const wallet = args.masterPrivateKey
        ? new ethers.Wallet(args.masterPrivateKey).connect(provider)
        : ethers.Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);

    const upgradeGatekeeper = new ethers.Contract(
        args.upgradeGatekeeperAddress,
        testContracts.upgradeGatekeeper.abi,
        wallet
    );

    console.log('Deploying new Governance target...');
    const newTargetGov = await deployContract(wallet, testContracts.governance, [], {
        gasLimit: 6500000
    });

    console.log('Deploying new Verifier target...');
    const newTargetVerifier = await deployContract(wallet, testContracts.verifier, [], {
        gasLimit: 6500000
    });

    console.log('Deploying new zkSync target...');
    const newTargetZkSync = await deployContract(wallet, testContracts.zkSync, [], {
        gasLimit: 6500000
    });

    console.log('Starting upgrade');
    await (
        await upgradeGatekeeper.startUpgrade(
            [newTargetGov.address, newTargetVerifier.address, newTargetZkSync.address],
            {
                gasLimit: 500000
            }
        )
    ).wait();

    // No need to proceed further if we don't want to finish or startPreparation
    if (!args.finish && !args.startPreparation) {
        return;
    }

    // wait notice period
    console.log('Waiting notice period');
    while (parseInt(await upgradeGatekeeper.upgradeStatus()) !== 2 /*Preparation*/) {
        await new Promise((r) => setTimeout(r, 1000));
        await (await upgradeGatekeeper.startPreparation({ gasLimit: 300000 })).wait();
    }

    if (!args.finish) {
        return;
    }

    const lastBlockInfo = JSON.parse(args.lastBlockInfo);
    const upgradeData = ethers.utils.defaultAbiCoder.encode([storedBlockInfoParam()], [lastBlockInfo]);

    console.log('Finishing upgrade');
    // finish upgrade
    await (await upgradeGatekeeper.finishUpgrade([[], [], upgradeData], { gasLimit: 3000000 })).wait();
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    })
    .finally(() => {});
