import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin, franklinContractCode, postContractToTesseracts} from "../src.ts/deploy";

async function main() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    const franklin = await deployFranklin(wallet, process.env.GENESIS_ROOT);
    await postContractToTesseracts(franklinContractCode, "Franklin", franklin.address);
    await franklin.setValidator(process.env.OPERATOR_ETH_ADDRESS, true);
    await addTestERC20Token(wallet, franklin);
}

main();
