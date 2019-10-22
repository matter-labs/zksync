import {bigNumberify} from "ethers/utils";
import {Contract, ethers} from "ethers";
import {governanceContractCode} from "../src.ts/deploy";
import {AddressZero} from "ethers/constants";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const governorWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const governanceDeployedContract = new Contract(process.env.GOVERNANCE_ADDR, governanceContractCode.interface, governorWallet);

async function main() {
    let tokenAddress = process.argv[process.argv.length - 1];
    console.log("Adding new ERC20 token to network: ", tokenAddress);
    let tx = await governanceDeployedContract.addToken(tokenAddress, {gasLimit: bigNumberify("1000000")});
    console.log("tx hash: ",tx.hash);
    let receipt = await tx.wait();
    console.log("status: ", receipt.status);
}

main();
