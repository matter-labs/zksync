import {ethers} from "ethers";
import {franklinContractCode, governanceContractCode} from "../src.ts/deploy";
import {Interface} from "ethers/utils";
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
const franklinInterface = new Interface(franklinContractCode.interface);
const governanceInterface = new Interface(governanceContractCode.interface);

function hex_to_ascii(str1) {
	const hex  = str1.toString();
	let str = "";
	for (let n = 0; n < hex.length; n += 2) {
		str += String.fromCharCode(parseInt(hex.substr(n, 2), 16));
	}
	return str;
 }

async function reason() {
    const args = process.argv.slice(2);
    const hash = args[0];
    console.log("tx hash:", hash);
    console.log("provider:", process.env.WEB3_URL);

    const tx = await provider.getTransaction(hash);
    if (!tx) {
        console.log("tx not found");
    } else {
        const parsedTransaction = franklinInterface.parseTransaction({data: tx.data});
        if (parsedTransaction) {
            console.log("parsed tx: ", parsedTransaction);
        } else {
            console.log('tx:', tx);
        }


        const receipt = await provider.getTransactionReceipt(hash);
        console.log("receipt:", receipt);
        console.log("\n \n ");

        if (receipt.gasUsed) {
            console.log("Gas used: ", receipt.gasUsed.toString());
        }

        if (receipt.status) {
            console.log("tx success");
        } else {
            const code = await provider.call(tx, tx.blockNumber);
            const reason = hex_to_ascii(code.substr(138));
            console.log("revert reason:", reason);
            console.log("revert code", code);
        }

        for (let log of receipt.logs){
            let parsedLog = franklinInterface.parseLog(log);
            if (!parsedLog) {
                parsedLog = governanceInterface.parseLog(log);
            }
            if (parsedLog) {
                console.log(parsedLog);
            } else {
                console.log(log);
            }
        }

    }
}

reason();
