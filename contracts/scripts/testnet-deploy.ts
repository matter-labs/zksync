import { ethers } from "ethers";
import { ArgumentParser } from "argparse";
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
    verifierContractCode
} from "../src.ts/deploy";

async function main() {
    const parser = new ArgumentParser({
        version: '0.0.1',
        addHelp: true,
        description: 'Deploy contracts and publish them on Etherscan/Tesseracts',
    });
    parser.addArgument('--deploy',  { action: 'storeTrue' });
    parser.addArgument('--publish', { action: 'storeTrue' });
    const args = parser.parseArgs(process.argv.slice(2));
    if (args.deploy == false && args.publish == false) {
        parser.printHelp();
        return;
    }

    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    const test_wallet = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY);

    let governanceAddress    = process.env.GOVERNANCE_ADDR;
    let priorityQueueAddress = process.env.PRIORITY_QUEUE_ADDR;
    let verifierAddress      = process.env.VERIFIER_ADDR;
    let franklinAddress      = process.env.CONTRACT_ADDR;

    let governanceConstructorArgs    = [ wallet.address ];
    let priorityQueueConstructorArgs = [ wallet.address ];
    let verifierConstructorArgs      = [];
    let franklinConstructorArgs      = [
        governanceAddress,
        verifierAddress,
        priorityQueueAddress,
        wallet.address,
        process.env.GENESIS_ROOT || ethers.constants.HashZero,
    ];

    if (args.deploy) {
        const governance = await deployGovernance(wallet, governanceContractCode, governanceConstructorArgs);
        governanceAddress = governance.address;
        
        const priorityQueue = await deployPriorityQueue(wallet, priorityQueueContractCode, priorityQueueConstructorArgs);
        priorityQueueAddress = priorityQueue.address;
        
        const verifier = await deployVerifier(wallet, verifierContractCode, verifierConstructorArgs);
        verifierAddress = verifier.address;
        
        franklinConstructorArgs = [
            governanceAddress,
            verifierAddress,
            priorityQueueAddress,
            wallet.address,
            process.env.GENESIS_ROOT || ethers.constants.HashZero,
        ];
        const franklin = await deployFranklin(wallet, franklinContractCode, franklinConstructorArgs);
        franklinAddress = franklin.address;

        await governance.setValidator(process.env.OPERATOR_ETH_ADDRESS, true);
        const erc20 = await addTestERC20Token(wallet, governance);
        await mintTestERC20Token(test_wallet, erc20);
    }

    if (args.publish) {
        try {
            if (process.env.ETH_NETWORK === 'localhost') {
                await postContractToTesseracts(governanceContractCode,    "Governance",    governanceAddress);
                await postContractToTesseracts(priorityQueueContractCode, "PriorityQueue", priorityQueueAddress);
                await postContractToTesseracts(verifierContractCode,      "Verifier",      verifierAddress);
                await postContractToTesseracts(franklinContractCode,      "Franklin",      franklinAddress);
            } else {
                await publishSourceCodeToEtherscan('Governance',    governanceAddress,    governanceContractSourceCode,    governanceContractCode,    governanceConstructorArgs);
                await publishSourceCodeToEtherscan('PriorityQueue', priorityQueueAddress, priorityQueueContractSourceCode, priorityQueueContractCode, priorityQueueConstructorArgs);
                await publishSourceCodeToEtherscan('Verifier',      verifierAddress,      verifierContractSourceCode,      verifierContractCode,      verifierConstructorArgs);
                await publishSourceCodeToEtherscan('Franklin',      franklinAddress,      franklinContractSourceCode,      franklinContractCode,      franklinConstructorArgs);
            }
        } catch (e) {
            console.error("Failed to post contract code: ", e.toString());
        }
    }
}

main();
