import { ArgumentParser } from 'argparse';
import { deployContract } from 'ethereum-waffle';
import { constants, ethers } from 'ethers';
import * as fs from 'fs';
import * as path from 'path';
import { web3Provider } from './utils';
import { readProductionContracts } from '../src.ts/deploy';

const { expect } = require('chai');

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));
const testContracts = readProductionContracts();

async function main() {
    const parser = new ArgumentParser({
        version: '0.0.1',
        addHelp: true,
        description: 'Contract upgrade'
    });
    parser.addArgument('contractAddress');
    parser.addArgument('upgradeGatekeeperAddress');
    const args = parser.parseArgs(process.argv.slice(2));
    if (process.env.CHAIN_ETH_NETWORK !== 'test') {
        console.log('Upgrading test contract not on test network is not allowed');
        process.exit(1);
    }

    const provider = web3Provider();

    const wallet = ethers.Wallet.fromMnemonic(ethTestConfig.test_mnemonic, "m/44'/60'/0'/0/0").connect(provider);

    const proxyContract = new ethers.Contract(args.contractAddress, testContracts.proxy.abi, wallet);

    const upgradeGatekeeper = new ethers.Contract(
        args.upgradeGatekeeperAddress,
        testContracts.upgradeGatekeeper.abi,
        wallet
    );

    const newTargetFranklin = await deployContract(wallet, testContracts.zkSync, [], {
        gasLimit: 6500000
    });

    console.log('Starting upgrade');
    await (
        await upgradeGatekeeper.startUpgrade([constants.AddressZero, constants.AddressZero, newTargetFranklin.address])
    ).wait();

    // wait notice period
    console.log('Waiting notice period');
    while (parseInt(await upgradeGatekeeper.upgradeStatus()) !== 2 /*Preparation*/) {
        await new Promise((r) => setTimeout(r, 1000));
        await (await upgradeGatekeeper.startPreparation({ gasLimit: 300000 })).wait();
    }

    console.log('Finish upgrade notice period');
    // finish upgrade
    await (await upgradeGatekeeper.finishUpgrade([[], [], []], { gasLimit: 300000 })).wait();

    await expect(await proxyContract.getTarget()).to.equal(newTargetFranklin.address, 'upgrade was unsuccessful');
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    })
    .finally(() => {});
