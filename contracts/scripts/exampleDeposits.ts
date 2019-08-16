import {franklinContractCode} from "../src.ts/deploy";
import {parseEther} from "ethers/utils";
import {Contract, ethers} from "ethers";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const franklinAddress = "010203040506070809101112131415161718192021222334252627";
const franklinAddressBinary = Buffer.from(franklinAddress, "hex");

async function main() {
    const franklinDeployedContract = new Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, wallet);
    // const depositValue = parseEther("0.3");
    // const depositFee = parseEther("0.01");
    // const tx = await franklinDeployedContract.depositETH(franklinAddressBinary, {value: depositValue});
    // const receipt = await tx.wait();
    let ethWallet2 = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);

    console.log(await franklinDeployedContract.blocks(4));
    console.log(await franklinDeployedContract.balances(ethWallet2.address, 0));
    // console.log(receipt);
}

main();