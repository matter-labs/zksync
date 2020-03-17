import {ethers} from "ethers";
import {ArgumentParser} from "argparse";
import { Deployer, addTestERC20Token, mintTestERC20Token } from "../src.ts/deploy";

async function main() {
    const parser = new ArgumentParser({
        version: '0.0.1',
        addHelp: true,
        description: 'Deploy contracts and publish them on Etherscan/Tesseracts',
    });
    parser.addArgument('--deploy', {action: 'storeTrue'});
    parser.addArgument('--publish', {action: 'storeTrue'});
    parser.addArgument('--test', {action: 'storeTrue'});
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
    const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    const testWallet = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/0").connect(provider);

    const deployer = new Deployer(wallet, args.test);

    if (args.deploy) {
        let timer = Date.now();
        await deployer.deployGovernance();
        console.log(`GOVERNANCE_TARGET_ADDR=${await deployer.getDeployedContract('GovernanceTarget').address}`);
        console.log(`GOVERNANCE_GENESIS_TX_HASH=${await deployer.getDeployTransactionHash('Governance')}`);
        console.log(`GOVERNANCE_ADDR=${await deployer.getDeployedContract('Governance').address}`);
        console.log(`Governance contract deployed, time: ${(Date.now() - timer) / 1000} secs`);

        timer = Date.now();
        await deployer.deployVerifier();
        console.log(`VERIFIER_TARGET_ADDR=${await deployer.getDeployedContract('VerifierTarget').address}`);
        console.log(`VERIFIER_ADDR=${await deployer.getDeployedContract('Verifier').address}`);
        console.log(`Verifier contract deployed, time: ${(Date.now() - timer) / 1000} secs`);

        timer = Date.now();
        await deployer.deployFranklin();
        console.log(`CONTRACT_TARGET_ADDR=${await deployer.getDeployedContract('FranklinTarget').address}`);
        console.log(`CONTRACT_GENESIS_TX_HASH=${await deployer.getDeployTransactionHash('Franklin')}`);
        console.log(`CONTRACT_ADDR=${await deployer.getDeployedContract('Franklin').address}`);
        console.log(`Main contract deployed, time: ${(Date.now() - timer) / 1000} secs`);

        const governance = await deployer.getDeployedContract('Governance');
        await governance.setValidator(process.env.OPERATOR_ETH_ADDRESS, true);

        const erc20 = await addTestERC20Token(wallet, governance);
        console.log("TEST_ERC20=" + erc20.address);
        await mintTestERC20Token(testWallet, erc20);
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
                ]);
            } else {
                // sequentially, since etherscan has request limit
                await deployer.publishSourceCodeToEtherscan("GovernanceTarget");
                await deployer.publishSourceCodeToEtherscan("VerifierTarget");
                await deployer.publishSourceCodeToEtherscan("FranklinTarget");
                await deployer.publishSourceCodeToEtherscan("Governance");
                await deployer.publishSourceCodeToEtherscan("Verifier");
                await deployer.publishSourceCodeToEtherscan("Franklin");
            }
        } catch (e) {
            console.error("Failed to post contract code: ", e.toString());
        }
    }
}

main();
