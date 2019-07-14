// import Contract2 from './build/Franklin'

const FrankliContract = require('../build/Franklin');
const ethers = require('ethers');


async function main() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    let wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

    let contract = new ethers.Contract(process.env.CONTRACT2_ADDR, FrankliContract.abi, provider).connect(wallet);

    var tx = {
        to: contract.address,
        value: ethers.utils.parseEther("0.1")
    };

    wallet.sendTransaction(tx).then(function(tx) {
        console.log(tx);
    });
}


main();