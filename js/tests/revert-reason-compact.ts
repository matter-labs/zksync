const ethers = require('ethers');
const ethersProvider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);

async function revertReason(hash) {
    const tx = await ethersProvider.getTransaction(hash);

    if (!tx) {
        return "tx not found";
    }
    
    const receipt = await ethersProvider.getTransactionReceipt(hash);

    if (receipt.status) {
        return "tx success";
    } 
    
    const code = await ethersProvider.call(tx, tx.blockNumber);

    if (code == '0x') {
        return 'empty revert reason';
    }
    
    return code
        .substr(138)
        .match(/../g)
        .map(h => parseInt(h, 16))
        .map(c => String.fromCharCode(c))
        .join('')
        .split('')
        .filter(c => /\w/.test(c))
        .join('');
}

async function run() {
    const txHash = process.argv[2];
    console.log(await revertReason(txHash));
}

run();
