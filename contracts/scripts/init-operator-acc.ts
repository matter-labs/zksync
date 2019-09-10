import 'ethers'
import {ethers} from "ethers";
import {Wallet} from "../../js/franklin_lib/src/wallet";

async function main() {
    const operatorAccountPrivateKey = process.env.OPERATOR_PRIVATE_KEY;
    let ethWallet = new ethers.Wallet(operatorAccountPrivateKey);
    let franklinWallet = await Wallet.fromEthWallet(ethWallet);
    console.log(`OPERATOR_ETH_ADDRESS=${ethWallet.address}`);
    console.log(`OPERATOR_FRANKLIN_ADDRESS=${franklinWallet.address}`);
}

main();
