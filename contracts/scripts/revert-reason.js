const ethers = require('ethers')
//const provider = ethers.getDefaultProvider('rinkeby')
//const provider = new ethers.providers.JsonRpcProvider('https://rinkeby.infura.io/v3/48beda66075e41bda8b124c6a48fdfa0')
const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL)

function hex_to_ascii(str1) {
	var hex  = str1.toString();
	var str = '';
	for (var n = 0; n < hex.length; n += 2) {
		str += String.fromCharCode(parseInt(hex.substr(n, 2), 16));
	}
	return str;
 }

async function reason() {
    var args = process.argv.slice(2)
    let hash = args[0]
    console.log('tx hash:', hash)
    console.log('provider:', process.env.WEB3_URL)

    let tx = await provider.getTransaction(hash)
    if (!tx) {
        console.log('tx not found')
    } else {
        //console.log('tx:', tx)

        let receipt = await provider.getTransactionReceipt(hash)
        //console.log('receipt:', receipt)

        if (receipt.status) {
            console.log('tx success')
        } else {
            let code = await provider.call(tx, tx.blockNumber)
            let reason = hex_to_ascii(code.substr(138))
            console.log('revert reason:', reason)
        }
    }
}

reason()