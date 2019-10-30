import {ethers} from "ethers";
import {
    addTestERC20Token,
    deployFranklin,
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
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    const governance = await deployGovernance(wallet, wallet.address, governanceContractCode, governanceContractSourceCode);
    const priorityQueue = await deployPriorityQueue(wallet, wallet.address, priorityQueueContractCode, priorityQueueContractSourceCode)
    const verifier = await deployVerifier(wallet, verifierContractCode, verifierContractSourceCode);
    const franklin = await deployFranklin(wallet, franklinContractCode, franklinContractSourceCode, governance.address, priorityQueue.address, verifier.address, wallet.address, process.env.GENESIS_ROOT);
    await governance.setValidator(process.env.OPERATOR_ETH_ADDRESS, true);
    await addTestERC20Token(wallet, governance);

    try {
        await postContractToTesseracts(governanceContractCode, "Governance", governance.address);
        await postContractToTesseracts(priorityQueueContractCode, "PriorityQueue", priorityQueue.address);
        await postContractToTesseracts(verifierContractCode, "Verifier", verifier.address);
        await postContractToTesseracts(franklinContractCode, "Franklin", franklin.address);
    } catch (e) {
        console.error("Failed to post contract code to Tesseracts: ", e.toString());
    }
}

main();
