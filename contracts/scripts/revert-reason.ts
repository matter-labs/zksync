import { ethers } from "ethers";
import { readContractCode, readProductionContracts } from "../src.ts/deploy";
import { Interface } from "ethers/lib/utils";
import * as chalk from "chalk";
const contracts = readProductionContracts();
const franklinInterface = new Interface(contracts.zkSync.abi);
const governanceInterface = new Interface(contracts.governance.abi);
const verifierInterface = new Interface(contracts.governance.abi);
const deployFactoryInterface = new Interface(readContractCode("DeployFactory").abi);

function hex_to_ascii(str1) {
    const hex = str1.toString();
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
        try {
            const parsedTransaction = franklinInterface.parseTransaction({ data: tx.data });
            if (parsedTransaction) {
                console.log("parsed tx: ", parsedTransaction);
            } else {
                console.log("tx:", tx);
            }
        } catch (e) {
            console.log("zkSync transaction is not parsed");
        }

        const transaction = await provider.getTransaction(hash);
        const receipt = await provider.getTransactionReceipt(hash);
        console.log("receipt:", receipt);
        console.log("\n \n ");

        if (receipt.gasUsed) {
            const gasLimit = transaction.gasLimit;
            const gasUsed = receipt.gasUsed;
            console.log("Gas limit: ", transaction.gasLimit.toString());
            console.log("Gas used: ", receipt.gasUsed.toString());

            // If more than 90% of gas was used, report it as an error.
            const threshold = gasLimit.mul(90).div(100);
            if (gasUsed >= threshold) {
                const error = chalk.bold.red;
                console.log(error("More than 90% of gas limit was used!"));
                console.log(error("It may be the reason of the transaction failure"));
            }
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
            try {
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
            } catch {}
        }
    }
}

reason();
