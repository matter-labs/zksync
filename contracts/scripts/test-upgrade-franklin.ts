import {ethers} from "ethers";
import {ArgumentParser} from "argparse";
import {proxyContractCode, upgradeGatekeeperTestContractCode} from "../src.ts/deploy";
import {deployContract} from "ethereum-waffle";
import {AddressZero} from "ethers/constants";

const {performance} = require('perf_hooks');
const {expect} = require("chai")

export const FranklinTestNoInitContractCode = require(`../build/FranklinTestNoInit`);

async function main() {
    try {
        const parser = new ArgumentParser({
            version: '0.0.1',
            addHelp: true,
            description: 'Contract upgrade',
        });
        parser.addArgument('contractAddress');
        parser.addArgument('upgradeGatekeeperAddress');
        const args = parser.parseArgs(process.argv.slice(2));
        if (process.env.ETH_NETWORK !== 'localhost') {
            console.log("Upgrading test contract not on localhost is not allowed");
            return;
        }

        const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
        if (process.env.ETH_NETWORK == "localhost") {
            // small polling interval for localhost network
            provider.pollingInterval = 200;
        }

        const wallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);

        const proxyContract = new ethers.Contract(
            args.contractAddress,
            proxyContractCode.interface,
            wallet
        );

        const upgradeGatekeeper = new ethers.Contract(
            args.upgradeGatekeeperAddress,
            upgradeGatekeeperTestContractCode.interface,
            wallet,
        );

        const newTargetFranklin = await deployContract(
            wallet,
            FranklinTestNoInitContractCode,
            [],
            {gasLimit: 6500000},
        );

        console.log("Starting upgrade");
        await (await upgradeGatekeeper.startUpgrade([AddressZero, AddressZero, newTargetFranklin.address])).wait();

        // wait notice period
        console.log("Waiting notice period");
        while (parseInt(await upgradeGatekeeper.upgradeStatus()) !== 2/*Preparation*/) {
            await new Promise(r => setTimeout(r, 1000));
            await (await upgradeGatekeeper.startPreparation({gasLimit: 300000})).wait();
        }

        console.log("Finish upgrade notice period");
        // finish upgrade
        await (await upgradeGatekeeper.finishUpgrade([[], [], []],  {gasLimit: 300000})).wait();

        await expect(await proxyContract.getTarget())
            .to.equal(newTargetFranklin.address, "upgrade was unsuccessful");
    } catch (e) {
        console.error(JSON.stringify(e));
        process.exit(0);
    }
}

main();
