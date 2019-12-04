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
    const test_wallet1 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY1);
    const test_wallet2 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY2);
    const test_wallet3 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY3);
    const test_wallet4 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY4);
    const test_wallet5 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY5);
    const test_wallet6 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY6);
    const test_wallet7 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY7);
    const test_wallet8 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY8);
    const test_wallet9 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY9);
    const test_wallet10 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY10);
    const test_wallet11 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY11);
    const test_wallet12 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY12);
    const test_wallet13 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY13);
    const test_wallet14 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY14);
    const test_wallet15 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY15);
    const test_wallet16 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY16);
    const test_wallet17 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY17);
    const test_wallet18 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY18);
    const test_wallet19 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY19);
    const test_wallet20 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY20);
    const test_wallet21 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY21);
    const test_wallet22 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY22);
    const test_wallet23 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY23);
    const test_wallet24 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY24);
    const test_wallet25 = new ethers.Wallet(process.env.TEST_ACCOUNT_PRIVATE_KEY25);

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
        await mintTestERC20Token(test_wallet1, erc20);
        await mintTestERC20Token(test_wallet2, erc20);
        await mintTestERC20Token(test_wallet3, erc20);
        await mintTestERC20Token(test_wallet4, erc20);
        await mintTestERC20Token(test_wallet5, erc20);
        await mintTestERC20Token(test_wallet6, erc20);
        await mintTestERC20Token(test_wallet7, erc20);
        await mintTestERC20Token(test_wallet8, erc20);
        await mintTestERC20Token(test_wallet9, erc20);
        await mintTestERC20Token(test_wallet10, erc20);
        await mintTestERC20Token(test_wallet11, erc20);
        await mintTestERC20Token(test_wallet12, erc20);
        await mintTestERC20Token(test_wallet13, erc20);
        await mintTestERC20Token(test_wallet14, erc20);
        await mintTestERC20Token(test_wallet15, erc20);
        await mintTestERC20Token(test_wallet16, erc20);
        await mintTestERC20Token(test_wallet17, erc20);
        await mintTestERC20Token(test_wallet18, erc20);
        await mintTestERC20Token(test_wallet19, erc20);
        await mintTestERC20Token(test_wallet20, erc20);
        await mintTestERC20Token(test_wallet21, erc20);
        await mintTestERC20Token(test_wallet22, erc20);
        await mintTestERC20Token(test_wallet23, erc20);
        await mintTestERC20Token(test_wallet24, erc20);
        await mintTestERC20Token(test_wallet25, erc20);
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
