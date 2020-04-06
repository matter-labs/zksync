import {bigNumberify} from "ethers/utils";
import {ethers} from "ethers";
import {Deployer} from "../src.ts/deploy";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const governorWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

async function main() {
    const deployer = new Deployer(governorWallet, false);
    let tokenAddress = process.argv[process.argv.length - 1];
    console.log("Adding new ERC20 token to network: ", tokenAddress);
    let tx = await deployer
        .getDeployedProxyContract('Governance')
        .addToken(tokenAddress, {gasLimit: bigNumberify("1000000")});
    console.log("tx hash: ", tx.hash);
    let receipt = await tx.wait();
    console.log("status: ", receipt.status);
}

main();
