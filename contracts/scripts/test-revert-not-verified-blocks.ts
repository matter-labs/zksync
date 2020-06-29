import {ArgumentParser} from "argparse";
import {deployContract} from "ethereum-waffle";
import {ethers} from "ethers";
import {AddressZero} from "ethers/constants";
import {readContractCode, readTestContracts} from "../src.ts/deploy";

const {performance} = require("perf_hooks");
const {expect} = require("chai");

const testContracts = readTestContracts();

async function main() {
    try {
        const parser = new ArgumentParser({
            version: "0.0.1",
            addHelp: true,
            description: "Contract blocks revert",
        });
        parser.addArgument("contractAddress");
        const args = parser.parseArgs(process.argv.slice(2));
        if (process.env.ETH_NETWORK !== "test") {
            console.log("Reverting test contract blocks not on test network is not allowed");
            process.exit(48);
            return;
        }

        const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

        const wallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);

        const ZkSyncContract = new ethers.Contract(
            args.contractAddress,
            testContracts.zkSync.interface,
            wallet,
        );

        console.log("Waiting expiration time");
        let expiration_time = parseInt(await ZkSyncContract.get_EXPECT_VERIFICATION_IN());
        await new Promise((r) => setTimeout(r, (expiration_time + 2) * 1000));

        console.log("Starting reverts");
        while (parseInt(await ZkSyncContract.totalBlocksCommitted()) != parseInt(await ZkSyncContract.totalBlocksVerified())) {
            await (await ZkSyncContract.revertBlocks(1)).wait();
        }
    } catch (e) {
        console.error(JSON.stringify(e));
        process.exit(72);
    }
}

main();
