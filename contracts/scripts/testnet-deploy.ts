import {ethers} from "ethers";
import {ArgumentParser} from "argparse";
import {
    addTestERC20Token,
    mintTestERC20Token,
    deployFranklin,
    publishSourceCodeToEtherscan,
    franklinContractSourceCode,
    franklinContractCode,
    deployGovernance,
    governanceContractSourceCode,
    governanceContractCode,
    postContractToTesseracts,
    deployVerifier,
    verifierContractSourceCode,
    verifierContractCode,
    governanceTestContractCode,
    verifierTestContractCode,
    franklinTestContractCode
} from "../src.ts/deploy";

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
    if (args.deploy == false && args.publish == false) {
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

    let governanceAddress = process.env.GOVERNANCE_ADDR;
    let priorityQueueAddress = process.env.PRIORITY_QUEUE_ADDR;
    let verifierAddress = process.env.VERIFIER_ADDR;
    let franklinAddress = process.env.CONTRACT_ADDR;

    let governanceConstructorArgs = [wallet.address];
    let priorityQueueConstructorArgs = [governanceAddress];
    let verifierConstructorArgs = [];
    let franklinConstructorArgs = [
        governanceAddress,
        verifierAddress,
        priorityQueueAddress,
        process.env.OPERATOR_FRANKLIN_ADDRESS,
        process.env.GENESIS_ROOT || ethers.constants.HashZero,
    ];

    if (args.deploy) {
        let timer = new Date().getTime();
        const governanceCode = args.test ? governanceTestContractCode : governanceContractCode;
        const governance = await deployGovernance(wallet, governanceCode, governanceConstructorArgs);
        console.log(`GOVERNANCE_GENESIS_TX_HASH=${governance.deployTransaction.hash}`);
        console.log(`GOVERNANCE_ADDR=${governance.address}`);
        console.log(`Governance contract deployed, time: ${(new Date().getTime() - timer) / 1000} secs`);
        governanceAddress = governance.address;

        timer = new Date().getTime();
        const verifierCode = args.test ? verifierTestContractCode : verifierContractCode;
        const verifier = await deployVerifier(wallet, verifierCode, verifierConstructorArgs);
        console.log(`VERIFIER_ADDR=${verifier.address}`);
        console.log(`Verifier contract deployed, time: ${(new Date().getTime() - timer) / 1000} secs`);
        verifierAddress = verifier.address;

        franklinConstructorArgs = [
            governanceAddress,
            verifierAddress,
            process.env.OPERATOR_FRANKLIN_ADDRESS.replace('sync:', '0x'),
            process.env.GENESIS_ROOT || ethers.constants.HashZero,
        ];
        timer = new Date().getTime();
        const franklinCode = args.test ? franklinTestContractCode : franklinContractCode;
        const franklin = await deployFranklin(wallet, franklinCode, franklinConstructorArgs);
        console.log(`CONTRACT_GENESIS_TX_HASH=${franklin.deployTransaction.hash}`);
        console.log(`CONTRACT_ADDR=${franklin.address}`);

        console.log(`Main contract deployed, time: ${(new Date().getTime() - timer) / 1000} secs`);
        franklinAddress = franklin.address;

        await governance.setValidator(process.env.OPERATOR_ETH_ADDRESS, true);

        const erc20 = await addTestERC20Token(wallet, governance);
        console.log("TEST_ERC20=" + erc20.address);
        await mintTestERC20Token(testWallet, erc20);

        if (args.publish) {
            try {
                if (process.env.ETH_NETWORK === 'localhost') {
                    await Promise.all([
                        postContractToTesseracts(governanceCode, "Governance", governanceAddress),
                        postContractToTesseracts(verifierCode, "Verifier", verifierAddress),
                        postContractToTesseracts(franklinCode, "Franklin", franklinAddress),
                    ]);
                } else {
                    await Promise.all([
                        publishSourceCodeToEtherscan('Governance', governanceAddress, governanceContractSourceCode, governanceContractCode, governanceConstructorArgs),
                        publishSourceCodeToEtherscan('Verifier', verifierAddress, verifierContractSourceCode, verifierContractCode, verifierConstructorArgs),
                        publishSourceCodeToEtherscan('Franklin', franklinAddress, franklinContractSourceCode, franklinContractCode, franklinConstructorArgs),
                    ]);
                }
            } catch (e) {
                console.error("Failed to post contract code: ", e.toString());
            }
        }

    }
}

main();
