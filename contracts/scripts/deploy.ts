import { ArgumentParser } from "argparse";
import { ethers, Wallet } from "ethers";
import { Deployer } from "../src.ts/deploy";
import { formatUnits, parseUnits } from "ethers/lib/utils";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

(async () => {
    const parser = new ArgumentParser({
        version: "0.1.0",
        addHelp: true,
        description: "Deploy contracts and publish them on Etherscan/Tesseracts",
    });
    parser.addArgument("--deployerPrivateKey", { required: false, help: "Wallet used to deploy contracts." });
    parser.addArgument("--governor", { required: false, help: "governor address" });

    parser.addArgument("--contract", {
        required: false,
        help: "Contract name: Governance, ZkSync, Verifier, Proxies or all by default.",
    });
    parser.addArgument("--gasPrice", { required: false, help: "Gas price in GWei." });
    parser.addArgument("--nonce", { required: false, help: "nonce (requires --contract argument)" });
    const args = parser.parseArgs(process.argv.slice(2));

    const wallet = args.deployerPrivateKey
        ? new Wallet(args.deployerPrivateKey, provider)
        : Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

    const gasPrice = args.gasPrice ? parseUnits(args.gasPrice, "gwei") : await provider.getGasPrice();
    console.log(`Using gas price: ${formatUnits(gasPrice, "gwei")} gwei`);

    if (args.nonce) {
        if (args.contract == null) {
            console.error("Nonce should be specified with --contract argument");
            process.exit(1);
        }
        console.log(`Using nonce: ${args.nonce}`);
    }

    const governorAddress = args.governor ? args.governor : wallet.address;
    console.log(`Deploying for governor: ${governorAddress}`);

    const deployer = new Deployer({ deployWallet: wallet, governorAddress, verbose: true });

    if (args.contract === "ZkSync" || args.contract == null) {
        await deployer.deployZkSyncTarget({ gasPrice, nonce: args.nonce });
    }

    if (args.contract === "Verifier" || args.contract == null) {
        await deployer.deployVerifierTarget({ gasPrice, nonce: args.nonce });
    }

    if (args.contract === "Governance" || args.contract == null) {
        await deployer.deployGovernanceTarget({ gasPrice, nonce: args.nonce });
    }

    if (args.contract === "Proxies" || args.contract == null) {
        await deployer.deployProxiesAndGatekeeper({ gasPrice, nonce: args.nonce });
    }
})();
