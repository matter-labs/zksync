import {ethers} from "ethers";
import {ArgumentParser} from "argparse";
import {Deployer} from "../src.ts/deploy";

async function main() {
    const parser = new ArgumentParser({
        version: '0.0.1',
        addHelp: true,
        description: 'Deploy contracts and publish them on Etherscan/Tesseracts',
    });
    parser.addArgument('--deploy', {action: 'storeTrue'});
    parser.addArgument('--publish', {action: 'storeTrue'});
    parser.addArgument('--test', {action: 'storeTrue'});
    parser.addArgument('--testkit', {action: 'storeTrue'});
    const args = parser.parseArgs(process.argv.slice(2));
    if (args.deploy == false && args.publish == false && args.test == false) {
        parser.printHelp();
        return;
    }
    if (process.env.ETH_NETWORK !== 'localhost' && args.test) {
        console.log("Deploying test contracts not on localhost is not allowed");
        return;
    }

    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    if (process.env.ETH_NETWORK == "localhost") {
        // small polling interval for localhost network
        provider.pollingInterval = 200;
    }
    let wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    let testWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);
    if (args.testkit) {
        wallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);
        testWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    }

    const deployer = new Deployer(wallet, args.test);

    if (args.deploy) {
        let timer = Date.now();
        await deployer.deployGovernance();
        console.log(`GOVERNANCE_TARGET_ADDR=${await deployer.getDeployedContract('GovernanceTarget').address}`);
        console.log(`GOVERNANCE_GENESIS_TX_HASH=${await deployer.getDeployTransactionHash('Governance')}`);
        console.log(`GOVERNANCE_ADDR=${await deployer.getDeployedProxyContract('Governance').address}`);
        console.log(`Governance contract deployed, time: ${(Date.now() - timer) / 1000} secs`);

        timer = Date.now();
        await deployer.deployVerifier();
        console.log(`VERIFIER_TARGET_ADDR=${await deployer.getDeployedContract('VerifierTarget').address}`);
        console.log(`VERIFIER_ADDR=${await deployer.getDeployedProxyContract('Verifier').address}`);
        console.log(`Verifier contract deployed, time: ${(Date.now() - timer) / 1000} secs`);

        timer = Date.now();
        await deployer.deployFranklin();
        console.log(`CONTRACT_TARGET_ADDR=${await deployer.getDeployedContract('FranklinTarget').address}`);
        console.log(`CONTRACT_GENESIS_TX_HASH=${await deployer.getDeployTransactionHash('Franklin')}`);
        console.log(`CONTRACT_ADDR=${await deployer.getDeployedProxyContract('Franklin').address}`);
        console.log(`Main contract deployed, time: ${(Date.now() - timer) / 1000} secs`);

        timer = Date.now();
        await deployer.deployUpgradeGatekeeper();
        console.log(`UPGRADE_GATEKEEPER_ADDR=${await deployer.getDeployedContract('UpgradeGatekeeper').address}`);
        console.log(`Upgrade gatekeeper deployed, time: ${(Date.now() - timer) / 1000} secs`);
        
        await deployer.setGovernanceValidator();
        
        const erc20 = await deployer.addTestERC20Token("GovernanceApprove");
        console.log("TEST_ERC20=" + erc20.address);
        await deployer.mintTestERC20Token(testWallet.address);
    }

    if (args.test) {
        await deployer.sendEthToTestWallets();
    }

    if (args.publish) {
        try {
            if (process.env.ETH_NETWORK === 'localhost') {
                await Promise.all([
                    deployer.postContractToTesseracts("GovernanceTarget"),
                    deployer.postContractToTesseracts("VerifierTarget"),
                    deployer.postContractToTesseracts("FranklinTarget"),
                    deployer.postContractToTesseracts("Governance"),
                    deployer.postContractToTesseracts("Verifier"),
                    deployer.postContractToTesseracts("Franklin"),
                    deployer.postContractToTesseracts("UpgradeGatekeeper"),
                ]);
            } else {
                // sequentially, since etherscan has request limit
                await deployer.publishSourceCodeToEtherscan("GovernanceTarget");
                await deployer.publishSourceCodeToEtherscan("VerifierTarget");
                await deployer.publishSourceCodeToEtherscan("FranklinTarget");
                await deployer.publishSourceCodeToEtherscan("Governance");
                await deployer.publishSourceCodeToEtherscan("Verifier");
                await deployer.publishSourceCodeToEtherscan("Franklin");
                await deployer.publishSourceCodeToEtherscan("UpgradeGatekeeper");
            }
        } catch (e) {
            console.error("Failed to post contract code: ", e.toString());
        }
    }
}

main();
