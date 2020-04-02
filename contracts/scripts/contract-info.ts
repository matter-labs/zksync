import {Deployer} from "../src.ts/deploy";
import {Contract, ethers} from "ethers";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

async function main() {
    const deployer = new Deployer(wallet, false);
    const franklinDeployedContract = deployer.getDeployedProxyContract('Franklin');
    let value = await franklinDeployedContract.onchainOps(2);
    console.log(value);
    value = await franklinDeployedContract.balancesToWithdraw(wallet.address, 0);
    console.log(value);
}

main();
