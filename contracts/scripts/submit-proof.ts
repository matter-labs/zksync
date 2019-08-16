import {bigNumberify} from "ethers/utils";
import {Contract, ethers} from "ethers";
import {franklinContractCode} from "../src.ts/deploy";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const franklinDeployedContract = new Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, wallet);

async function main() {
    const blockProof = [0, 0, 0, 0, 0, 0, 0, 0];
    let tx = await franklinDeployedContract.verifyBlock(1, blockProof, {gasLimit: bigNumberify("100000")});
    console.log("tx: ",tx);
    let receipt = await tx.wait();
    console.log("receipt :", receipt);
}

main();
