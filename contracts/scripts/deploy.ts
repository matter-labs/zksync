import {ethers} from "ethers";
import {ArgumentParser} from "argparse";
import {deployBySteps, Deployer} from "../src.ts/deploy";

(async () => {
    const parser = new ArgumentParser({
        version: '0.0.1',
        addHelp: true,
        description: 'Deploy contracts and publish them on Etherscan/Tesseracts',
    });
    parser.addArgument('--deployStep', {required: true});
    parser.addArgument('--deployerPrivateKey', {required: false});
    parser.addArgument('--governorAddress', {required: false});
    const args = parser.parseArgs(process.argv.slice(2));

    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

    if (process.env.ETH_NETWORK === "localhost") {
        provider.pollingInterval = 200;
        const localDeployWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1");
        args.deployerPrivateKey = localDeployWallet.privateKey;
        args.governorAddress = localDeployWallet.address;
    } else {
        let exit = false;
        if (args.deployerPrivateKey == null) {
            console.error("Deployer private key should be specified for non local deployments");
            exit = true;
        }
        if (args.governorAddress == null) {
            console.error("Governor address should be specified for non local deployments");
            exit = true;
        }
        if (exit) {
            process.exit(1);
        }
    }

    const deployWallet = new ethers.Wallet(args.deployerPrivateKey, provider);

    let deployStep = 0;
    try {
        deployStep = parseInt(args.deployStep);
        if (deployStep < 0 || deployStep > 8 || isNaN(deployStep)) {
            console.error('Deploy step should be number (0-8)"');
            process.exit(1);
        }
    } catch (e) {
        console.error("Deploy step should be number (0-8)");
        process.exit(1);
    }

    await deployBySteps(deployWallet, deployStep, false, true);
})();

//
// if (args.test) {
//     await deployer.sendEthToTestWallets();
// }

// if (args.publish) {
//     try {
//         if (process.env.ETH_NETWORK === 'localhost') {
//             await Promise.all([
//                 deployer.postContractToTesseracts("GovernanceTarget"),
//                 deployer.postContractToTesseracts("VerifierTarget"),
//                 deployer.postContractToTesseracts("FranklinTarget"),
//                 deployer.postContractToTesseracts("Governance"),
//                 deployer.postContractToTesseracts("Verifier"),
//                 deployer.postContractToTesseracts("Franklin"),
//                 deployer.postContractToTesseracts("UpgradeGatekeeper"),
//             ]);
//         } else {
//             // sequentially, since etherscan has request limit
//             await deployer.publishSourceCodeToEtherscan("GovernanceTarget");
//             await deployer.publishSourceCodeToEtherscan("VerifierTarget");
//             await deployer.publishSourceCodeToEtherscan("FranklinTarget");
//             await deployer.publishSourceCodeToEtherscan("Governance");
//             await deployer.publishSourceCodeToEtherscan("Verifier");
//             await deployer.publishSourceCodeToEtherscan("Franklin");
//             await deployer.publishSourceCodeToEtherscan("UpgradeGatekeeper");
//         }
//     } catch (e) {
//         console.error("Failed to post contract code: ", e.toString());
//     }
// }
// }

