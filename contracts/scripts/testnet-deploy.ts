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
    deployPriorityQueue,
    priorityQueueContractSourceCode,
    priorityQueueContractCode,
    deployVerifier,
    verifierContractSourceCode,
    verifierContractCode,
    governanceTestContractCode,
    priorityQueueTestContractCode,
    verifierTestContractCode,
    franklinTestContractCode,
    proxyContractCode,
    proxyContractSourceCode,
    proxyTestContractCode,
    deployProxy,
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

    let governanceInitArgs = ["address"];
    let governanceInitArgsValues = [wallet.address];
    let priorityQueueInitArgs = ["address"];
    let priorityQueueInitArgsValues = [governanceAddress];
    let verifierInitArgs = [];
    let verifierInitArgsValues = [];

    if (args.deploy) {
        let timer = new Date().getTime();
        const proxyCode = args.test ? proxyTestContractCode : proxyContractCode;

        const governanceCode = args.test ? governanceTestContractCode : governanceContractCode;
        let governance, governanceAddressDeployed;
        [governance, governanceAddressDeployed] = await deployGovernance(
            wallet,
            proxyCode,
            governanceCode,
            governanceInitArgs,
            governanceInitArgsValues,
        );
        console.log(`Governance contract deployed, time: ${(new Date().getTime() - timer) / 1000} secs`);
        governanceAddress = governanceAddressDeployed;

        timer = new Date().getTime();
        const priorityQueueCode = args.test ? priorityQueueTestContractCode : priorityQueueContractCode;
        let priorityQueue, priorityQueueAddressDeployed;
        [priorityQueue, priorityQueueAddressDeployed] = await deployPriorityQueue(
            wallet,
            proxyCode,
            priorityQueueCode,
            priorityQueueInitArgs,
            priorityQueueInitArgsValues,
        );
        console.log(`Priority queue contract deployed, time: ${(new Date().getTime() - timer) / 1000} secs`);
        priorityQueueAddress = priorityQueueAddressDeployed;

        timer = new Date().getTime();
        const verifierCode = args.test ? verifierTestContractCode : verifierContractCode;
        let verifier, verifierAddressDeployed;
        [verifier, verifierAddressDeployed] = await deployVerifier(
            wallet,
            proxyCode,
            verifierCode,
            verifierInitArgs,
            verifierInitArgsValues,
        );
        console.log(`Verifier contract deployed, time: ${(new Date().getTime() - timer) / 1000} secs`);
        verifierAddress = verifierAddressDeployed;

        let franklinInitArgs = [
            "address",
            "address",
            "address",
            "address",
            "bytes32",
        ];
        let franklinInitArgsValues = [
            governance.address,
            verifier.address,
            priorityQueue.address,
            process.env.OPERATOR_FRANKLIN_ADDRESS.replace("sync:", "0x"),
            process.env.GENESIS_ROOT || ethers.constants.HashZero,
        ];
        timer = new Date().getTime();
        const franklinCode = args.test ? franklinTestContractCode : franklinContractCode;
        let franklin, franklinAddressDeployed;
        [franklin, franklinAddressDeployed] = await deployFranklin(
            wallet,
            proxyCode,
            franklinCode,
            franklinInitArgs,
            franklinInitArgsValues,
        );
        console.log(`Main contract deployed, time: ${(new Date().getTime() - timer) / 1000} secs`);
        franklinAddress = franklinAddressDeployed;

        await governance.setValidator(process.env.OPERATOR_ETH_ADDRESS, true);

        const erc20 = await addTestERC20Token(wallet, governance);
        await mintTestERC20Token(testWallet, erc20);

        if (args.publish) {
            try {
                if (process.env.ETH_NETWORK === 'localhost') {
                    await Promise.all([
                        postContractToTesseracts(governanceCode, "Governance", governanceAddress),
                        postContractToTesseracts(priorityQueueCode, "PriorityQueue", priorityQueueAddress),
                        postContractToTesseracts(verifierCode, "Verifier", verifierAddress),
                        postContractToTesseracts(franklinCode, "Franklin", franklinAddress),
                    ]);
                } else {
                    await Promise.all([
                        publishSourceCodeToEtherscan('GovernanceProxy', governance.address, proxyContractSourceCode, proxyContractCode, []),
                        publishSourceCodeToEtherscan('PriorityQueueProxy', priorityQueue.address, proxyContractSourceCode, proxyContractCode, []),
                        publishSourceCodeToEtherscan('VerifierProxy', verifier.address, proxyContractSourceCode, proxyContractCode, []),
                        publishSourceCodeToEtherscan('FranklinProxy', franklin.address, proxyContractSourceCode, proxyContractCode, []),

                        publishSourceCodeToEtherscan('Governance', governanceAddress, governanceContractSourceCode, governanceContractCode, []),
                        publishSourceCodeToEtherscan('PriorityQueue', priorityQueueAddress, priorityQueueContractSourceCode, priorityQueueContractCode, []),
                        publishSourceCodeToEtherscan('Verifier', verifierAddress, verifierContractSourceCode, verifierContractCode, []),
                        publishSourceCodeToEtherscan('Franklin', franklinAddress, franklinContractSourceCode, franklinContractCode, []),
                    ]);
                }
            } catch (e) {
                console.error("Failed to post contract code: ", e.toString());
            }
        }

    }
}

main();
