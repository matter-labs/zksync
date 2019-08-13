import 'ethers'
import {ethers} from "ethers";
import {Wallet} from "../../js/franklin_lib/src/wallet";

async function main() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    const operatorAccountPrivateKey = process.env.OPERATOR_PRIVATE_KEY;
    console.log(operatorAccountPrivateKey);
    const operatorAccountPassword = process.env.OPERATOR_ETH_ACCOUNT_PASSWORD;
    let new_acc_address = await provider.send("personal_importRawKey", [operatorAccountPrivateKey, operatorAccountPassword]);
    let signer = await provider.getSigner(new_acc_address);
    await signer.unlock(operatorAccountPassword);
    let franklinWallet = await Wallet.fromEthWallet(signer);
    console.log(`OPERATOR_FRANKLIN_ADDRESS=${franklinWallet.address}`);
    console.log(`OPERATOR_ETH_ADDRESS=${new_acc_address}`);

}

main();