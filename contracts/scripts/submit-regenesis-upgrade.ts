import { ArgumentParser } from 'argparse';
import { deployContract } from 'ethereum-waffle';
import { Contract, ethers } from 'ethers';
import * as fs from 'fs';
import * as path from 'path';
import { web3Provider, storedBlockInfoParam } from './utils';
import { readProductionContracts } from '../src.ts/deploy';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));
const testContracts = readProductionContracts();

async function startUpgrade(wallet: ethers.Wallet, upgradeGatekeeper: Contract) {
    console.log('Deploying new Governance target...');
    const newTargetGov = await deployContract(wallet, testContracts.governance, [], {
        gasLimit: 6500000
    });

    console.log(`CONTRACTS_GOVERNANCE_TARGET_ADDR=${newTargetGov.address}`);
    console.log('Deploying new Verifier target...');
    const newTargetVerifier = await deployContract(wallet, testContracts.verifier, [], {
        gasLimit: 6500000
    });

    console.log(`CONTRACTS_VERIFIER_TARGET_ADDR=${newTargetVerifier.address}`);
    const newTargetZkSync = await deployContract(wallet, testContracts.zkSync, [], {
        gasLimit: 6500000
    });

    console.log(`CONTRACTS_CONTRACT_TARGET_ADDR=${newTargetZkSync.address}`);
    console.log('Starting upgrade...');
    await (
        await upgradeGatekeeper.startUpgrade(
            [newTargetGov.address, newTargetVerifier.address, newTargetZkSync.address],
            {
                gasLimit: 500000
            }
        )
    ).wait();
    console.log('Upgrade has been successfully started.');
}

async function startPreparation(upgradeGatekeeper: Contract) {
    console.log('Trying to start preparation...');
    while (parseInt(await upgradeGatekeeper.upgradeStatus()) !== 2 /*Preparation*/) {
        await new Promise((r) => setTimeout(r, 1000));
        await (await upgradeGatekeeper.startPreparation({ gasLimit: 300000 })).wait();
    }
    console.log('Upgrade preparation has been successfully started');
}

async function finishUpgrade(upgradeGatekeeper: Contract, lastBlockInfo: string) {
    const blockInfo = JSON.parse(lastBlockInfo);
    const upgradeData = ethers.utils.defaultAbiCoder.encode([storedBlockInfoParam()], [blockInfo]);

    console.log('Finishing upgrade');
    await (await upgradeGatekeeper.finishUpgrade([[], [], upgradeData], { gasLimit: 3000000 })).wait();
    console.log('The upgrade has finished');
}

async function cancelUpgrade(upgradeGatekeeper: Contract) {
    await (
        await upgradeGatekeeper.cancelUpgrade({
            gasLimit: 500000
        })
    ).wait();
}

async function main() {
    const parser = new ArgumentParser({
        version: '0.0.1',
        addHelp: true,
        description: 'Contract upgrade'
    });
    parser.addArgument('--masterPrivateKey');
    parser.addArgument('--upgradeGatekeeperAddress');
    parser.addArgument('--lastBlockInfo');
    parser.addArgument('--startUpgrade');
    parser.addArgument('--finishUpgrade');
    parser.addArgument('--startPreparation');
    parser.addArgument('--cancelUpgrade');
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

    if (!args.startUpgrade && !args.startPreparation && !args.finishUpgrade && !args.cancelUpgrade) {
        console.log(`Please supply at least one of the following flags:
        --startUpgrade,
        --startPreparation,
        --finishUpgrade
        `);
        return;
    }

    if (args.startUpgrade) {
        await startUpgrade(wallet, upgradeGatekeeper);
    }

    if (args.startPreparation) {
        await startPreparation(upgradeGatekeeper);
    }

    if (args.finishUpgrade) {
        await finishUpgrade(upgradeGatekeeper, args.lastBlockInfo);
    }

    if (args.cancelUpgrade) {
        await cancelUpgrade(upgradeGatekeeper);
    }
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    })
    .finally(() => {});
