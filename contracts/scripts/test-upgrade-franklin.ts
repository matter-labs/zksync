import { ArgumentParser } from 'argparse';
import { deployContract } from 'ethereum-waffle';
import { constants, ethers } from 'ethers';
import * as fs from 'fs';
import * as path from 'path';
import { web3Provider, storedBlockInfoParam } from './utils';
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
    parser.addArgument('--contractAddress');
    parser.addArgument('--upgradeGatekeeperAddress');
    parser.addArgument('--lastBlockInfo');
    const args = parser.parseArgs(process.argv.slice(2));
    // if (process.env.CHAIN_ETH_NETWORK !== 'test') {
    //     console.log('Upgrading test contract not on test network is not allowed');
    //     process.exit(1);
    // }

    const lastBlockInfo = JSON.parse(args.lastBlockInfo);
    console.log(lastBlockInfo);
    const encodedStoredBlockInfo = ethers.utils.defaultAbiCoder.encode([storedBlockInfoParam()], [lastBlockInfo]);

    console.log(encodedStoredBlockInfo);

    const provider = web3Provider();

    const wallet = ethers.Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);

    const proxyContract = new ethers.Contract(args.contractAddress, testContracts.proxy.abi, wallet);

    const upgradeGatekeeper = new ethers.Contract(
        args.upgradeGatekeeperAddress,
        testContracts.upgradeGatekeeper.abi,
        wallet
    );

    const newTargetFranklin = await deployContract(wallet, testContracts.zkSync, [], {
        gasLimit: 6500000
    });

    const newTargetGov = await deployContract(wallet, testContracts.governance, [], {
        gasLimit: 6500000
    });

    console.log('Starting upgrade');
    console.log(wallet.address);
    console.log(await upgradeGatekeeper.getMaster());
    // await (
    //     await upgradeGatekeeper.cancelUpgrade()
    // ).wait();
    await (
        await upgradeGatekeeper.startUpgrade([newTargetGov.address, constants.AddressZero, newTargetFranklin.address], {
            gasLimit: 500000
        })
    ).wait();

    // wait notice period
    console.log('Waiting notice period');
    while (parseInt(await upgradeGatekeeper.upgradeStatus()) !== 2 /*Preparation*/) {
        await new Promise((r) => setTimeout(r, 1000));
        await (await upgradeGatekeeper.startPreparation({ gasLimit: 300000 })).wait();
    }

    console.log(await proxyContract.getMaster());
    console.log(upgradeGatekeeper.address);
    console.log('Finish upgrade notice period');
    //  console.log(await proxyContract.exodusMode());
    // finish upgrade
    await (await upgradeGatekeeper.finishUpgrade([[], [], encodedStoredBlockInfo], { gasLimit: 3000000 })).wait();

    await expect(await proxyContract.getTarget()).to.equal(newTargetFranklin.address, 'upgrade was unsuccessful');
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    })
    .finally(() => {});
