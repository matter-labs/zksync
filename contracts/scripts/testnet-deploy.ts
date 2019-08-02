import {ethers} from "ethers";
import {addTestERC20Token, deployFranklin} from "../src.ts/deploy";

async function main() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    const franklin = await deployFranklin(wallet);
    await addTestERC20Token(wallet, franklin);
}

main();
