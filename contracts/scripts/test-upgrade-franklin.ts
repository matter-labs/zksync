import {ethers} from "ethers";
import {ArgumentParser} from "argparse";
import {proxyContractCode, upgradeGatekeeperTestContractCode} from "../src.ts/deploy";
import {deployContract} from "ethereum-waffle";

const {performance} = require('perf_hooks');
const {expect} = require("chai")

export const FranklinTestNoInitContractCode = require(`../build/FranklinTestNoInit`);

async function main() {
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

    const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

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

    const newTarget = await deployContract(
        wallet,
        FranklinTestNoInitContractCode,
        [],
        {gasLimit: 6500000},
    );

    let notice_period = parseInt(await upgradeGatekeeper.get_NOTICE_PERIOD());

    await (await upgradeGatekeeper.startProxyUpgrade(proxyContract.address, newTarget.address)).wait();

    // wait notice period
    await new Promise(r => setTimeout(r, notice_period * 1000 + 10));

    // finish upgrade
    await (await upgradeGatekeeper.activateCleaningUpStatusOfUpgrade(proxyContract.address)).wait();
    await (await upgradeGatekeeper.finishProxyUpgrade(proxyContract.address, [])).wait();

    await expect(await proxyContract.getTarget())
        .to.equal(newTarget.address);
}

main();
