import { ArgumentParser } from "argparse";
import { ethers, Wallet } from "ethers";
import { Deployer } from "../src.ts/deploy";
import { formatUnits, parseUnits } from "ethers/lib/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

(async () => {
    const parser = new ArgumentParser({
        version: "0.1.0",
        addHelp: true,
        description: "Deploy new contracts and upgrade testnet proxy Tesseracts",
    });
    parser.addArgument("--deployerPrivateKey", { required: false, help: "Wallet used to deploy contracts." });
    parser.addArgument("--governor", { required: false, help: "governor address" });

    parser.addArgument("--contracts", {
        required: false,
        help: "Contracts to upgrade (one or more comma-separated): Governance,Verifier,ZkSync, or all by default.",
        defaultValue: "Governance,Verifier,ZkSync",
    });
    parser.addArgument("--initArgs", {
        required: false,
        help:
            "Upgrade function parameters comma-separated, RLP serialized in hex (Governance,Verifier,ZkSync): 0xaa..aa,0xbb..bb,0xcc..c or zero by default.",
        defaultValue: "0x,0x,0x",
    });
    parser.addArgument("--cancelPreviousUpgrade", {
        required: false,
        help: "cancels pending upgrade",
        action: "storeTrue",
    });
    parser.addArgument("--gasPrice", { required: false, help: "Gas price in GWei." });
    parser.addArgument("--nonce", { required: false, help: "nonce (requires --contract argument)" });
    const args = parser.parseArgs(process.argv.slice(2));

    const wallet = args.deployerPrivateKey
        ? new Wallet(args.deployerPrivateKey, provider)
        : Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

    const gasPrice = args.gasPrice ? parseUnits(args.gasPrice, "gwei") : await provider.getGasPrice();
    console.info(`Using gas price: ${formatUnits(gasPrice, "gwei")} gwei`);

    if (args.nonce) {
        console.info(`Using nonce: ${args.nonce}`);
    }

    const governorAddress = args.governor ? args.governor : wallet.address;
    console.info(`Deploying for governor: ${governorAddress}`);

    const deployer = new Deployer({ deployWallet: wallet, governorAddress, verbose: true });
    console.info(`Upgrading ${args.contracts} contracts`);
    const upgradeTargets = [ethers.constants.AddressZero, ethers.constants.AddressZero, ethers.constants.AddressZero];

    for (const contract of args.contracts.split(",")) {
        if (contract !== "Governance" && contract !== "Verifier" && contract !== "ZkSync") {
            console.error(`Unknow upgrade target: ${contract}`);
            process.exit(1);
        }

        if (contract === "Governance") {
            await deployer.deployGovernanceTarget({ gasPrice, nonce: args.nonce });
            if (args.nonce != null) {
                ++args.nonce;
            }
            upgradeTargets[0] = deployer.addresses.GovernanceTarget;
        }
        if (contract === "Verifier") {
            await deployer.deployVerifierTarget({ gasPrice, nonce: args.nonce });
            if (args.nonce != null) {
                ++args.nonce;
            }
            upgradeTargets[1] = deployer.addresses.VerifierTarget;
        }
        if (contract === "ZkSync") {
            await deployer.deployZkSyncTarget({ gasPrice, nonce: args.nonce });
            if (args.nonce != null) {
                ++args.nonce;
            }
            upgradeTargets[2] = deployer.addresses.ZkSyncTarget;
        }
    }
    const upgradeGatekeeper = deployer.upgradeGatekeeperContract(wallet);

    if (args.cancelPreviousUpgrade) {
        const cancelUpgradeTx = await upgradeGatekeeper.cancelUpgrade({
            gasPrice,
            gasLimit: 500000,
            nonce: args.nonce,
        });
        if (args.nonce != null) {
            ++args.nonce;
        }
        console.info(`Canceling pending upgrade: ${cancelUpgradeTx.hash}`);
        await cancelUpgradeTx.wait();
        console.info("Pending upgrade canceled");
    }

    const startUpgradeTx = await upgradeGatekeeper.startUpgrade(upgradeTargets, {
        gasPrice,
        gasLimit: 500000,
        nonce: args.nonce,
    });
    if (args.nonce != null) {
        ++args.nonce;
    }
    console.info(`Upgrade started: ${startUpgradeTx.hash}`);
    await startUpgradeTx.wait();

    const startPreparationUpgradeTx = await upgradeGatekeeper.startPreparation({
        gasPrice,
        gasLimit: 500000,
        nonce: args.nonce,
    });
    if (args.nonce != null) {
        ++args.nonce;
    }
    console.info(`Upgrade preparation tx: ${startPreparationUpgradeTx.hash}`);
    await startPreparationUpgradeTx.wait();

    const initArgs = args.initArgs.split(",");
    const finishUpgradeTx = await upgradeGatekeeper.finishUpgrade(initArgs, {
        gasPrice,
        gasLimit: 500000,
        nonce: args.nonce,
    });
    console.info(`Upgrade finish tx: ${finishUpgradeTx.hash}`);
    await finishUpgradeTx.wait();

    console.info("Upgrade successful");
})();
