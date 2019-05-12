const ethers = require('ethers')

const provider = new ethers.providers.JsonRpcProvider()
const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms))

let source = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/0").connect(provider)
let sourceNonce = null

const PlasmaContractABI = require('../abi/PlasmaContract.json').abi
const contract = new ethers.Contract(process.env.CONTRACT_ADDR, PlasmaContractABI, provider)
const paddingPubKey = JSON.parse(process.env.PADDING_PUB_KEY);

(async function() {

    sourceNonce = await source.getTransactionCount("pending")

    console.log('starting...')
    
    // First 4 bytes of the hash of "fee()" for the sighash selector
    //let data = ethers.utils.hexDataSlice(ethers.utils.id('exitor()'), 0, 4);
    let data = ethers.utils.hexDataSlice(ethers.utils.id('x()'), 0, 4);
    let to = process.env.CONTRACT_ADDR
    let tx = {to, data}

    try {
        let r = await provider.call(tx);
        console.log('r', r)
    } catch (error) {
        // error.reason now populated with an REVERT reason
        console.log("Failure reason:", error);
    }

})()
