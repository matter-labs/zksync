import {ethers} from "ethers";
import {Interface} from "ethers/utils";
import {readContractCode, readProductionContracts} from "../src.ts/deploy";
const contracts = readProductionContracts();
const franklinInterface = new Interface(contracts.zkSync.interface);
const governanceInterface = new Interface(contracts.governance.interface);
const verifierInterface = new Interface(contracts.governance.interface);
const deployFactoryInterface = new Interface(readContractCode("DeployFactory").interface);

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
    const web3 = args[1] == null ? process.env.WEB3_URL : args[1];
    console.log("tx hash:", hash);
    console.log("provider:", web3);

    const provider = new ethers.providers.JsonRpcProvider(web3);


    const tx = await provider.getTransaction(hash);
    if (!tx) {
        console.log("tx not found");
    } else {
        const parsedTransaction = franklinInterface.parseTransaction({data: tx.data});
        if (parsedTransaction) {
            console.log("parsed tx: ", parsedTransaction);
        } else {
            console.log("tx:", tx);
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

        for (const log of receipt.logs) {
            let parsedLog = franklinInterface.parseLog(log);
            if (!parsedLog) {
                parsedLog = governanceInterface.parseLog(log);
            }
            if (!parsedLog) {
                parsedLog = verifierInterface.parseLog(log);
            }
            if (!parsedLog) {
                parsedLog = deployFactoryInterface.parseLog(log);
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
