import { Contract, ethers } from "ethers";
import { Deployer } from "../src.ts/deploy";

const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

async function main() {
    const deployer = new Deployer({ deployWallet: wallet });
    const upgradeGatekeeper = deployer.upgradeGatekeeperContract(wallet);
    const tx = await upgradeGatekeeper.finishUpgrade(["0x", "0x", "0x"]);
    console.log(tx);
}

main();
