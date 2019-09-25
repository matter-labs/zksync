import {ethers} from "ethers";
import {
    addTestERC20Token,
    deployFranklin,
    franklinContractCode,
    deployGovernance,
    governanceContractCode,
    postContractToTesseracts,
    deployPriorityQueue, priorityQueueContractCode, deployVerifier, verifierContractCode
} from "../src.ts/deploy";

async function main() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    const governance = await deployGovernance(wallet, wallet.address, governanceContractCode);
    const priorityQueue = await deployPriorityQueue(wallet, wallet.address, priorityQueueContractCode)
    const verifier = await deployVerifier(wallet, verifierContractCode);
    const franklin = await deployFranklin(wallet, governance.address, priorityQueue.address, verifier.address, franklinContractCode, process.env.GENESIS_ROOT);
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
