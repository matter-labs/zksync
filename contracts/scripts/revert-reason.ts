import {ethers} from "ethers";
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

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
        // console.log('tx:', tx)

        const receipt = await provider.getTransactionReceipt(hash);

        if (receipt.gasUsed) {
            console.log("Gas used: ", receipt.gasUsed.toString());
        }

        if (receipt.status) {
            console.log("tx success");
        } else {
            const code = await provider.call(tx, tx.blockNumber);
            const reason = hex_to_ascii(code.substr(138));
            console.log("revert reason:", reason);
        }
    }
}

reason();
