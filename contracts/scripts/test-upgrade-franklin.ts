import { ArgumentParser } from "argparse";
import { deployContract } from "ethereum-waffle";
import { constants, ethers } from "ethers";
import { readTestContracts } from "../src.ts/deploy";

const { expect } = require("chai");

export const FranklinTestUpgradeTargetContractCode = require(`../build/ZkSyncTestUpgradeTarget`);

const testContracts = readTestContracts();

async function main() {
    try {
        const parser = new ArgumentParser({
            version: "0.0.1",
            addHelp: true,
            description: "Contract upgrade",
        });
        parser.addArgument("contractAddress");
        parser.addArgument("upgradeGatekeeperAddress");
        const args = parser.parseArgs(process.argv.slice(2));
        if (process.env.ETH_NETWORK !== "test") {
            console.log("Upgrading test contract not on test network is not allowed");
            process.exit(1);
            return;
        }

        const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

        const wallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);

        const proxyContract = new ethers.Contract(args.contractAddress, testContracts.proxy.abi, wallet);

        const upgradeGatekeeper = new ethers.Contract(
            args.upgradeGatekeeperAddress,
            testContracts.upgradeGatekeeper.abi,
            wallet
        );

        const newTargetFranklin = await deployContract(wallet, FranklinTestUpgradeTargetContractCode, [], {
            gasLimit: 6500000,
        });

        console.log("Starting upgrade");
        await (
            await upgradeGatekeeper.startUpgrade([
                constants.AddressZero,
                constants.AddressZero,
                newTargetFranklin.address,
            ])
        ).wait();

        // wait notice period
        console.log("Waiting notice period");
        while (parseInt(await upgradeGatekeeper.upgradeStatus()) !== 2 /*Preparation*/) {
            await new Promise((r) => setTimeout(r, 1000));
            await (await upgradeGatekeeper.startPreparation({ gasLimit: 300000 })).wait();
        }

        console.log("Finish upgrade notice period");
        // finish upgrade
        await (await upgradeGatekeeper.finishUpgrade([[], [], []], { gasLimit: 300000 })).wait();

        await expect(await proxyContract.getTarget()).to.equal(newTargetFranklin.address, "upgrade was unsuccessful");
    } catch (e) {
        console.error(JSON.stringify(e));
        process.exit(1);
    }
}

main();
