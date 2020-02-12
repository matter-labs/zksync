const ethers = require("ethers")
const { expect, use } = require("chai")
const { createMockProvider, getWallets, solidity, deployContract } = require("ethereum-waffle");
const { bigNumberify, parseEther, hexlify, formatEther } = require("ethers/utils");

// For: geth
// const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
// const wallet: any = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

// For: ganache
const provider = createMockProvider();
const [wallet, exitWallet]  = getWallets(provider);

use(solidity);

async function deployTestContract(file) {
    try {
        return await deployContract(wallet, require(file), [], {
            gasLimit: 6000000,
        })
    } catch (err) {
        console.log('Error deploying', file, ': ', err)
    }
}

async function getCallRevertReason(f) {
    let revertReason = "VM did not revert"
    try {
        let r = await f()
    } catch(e) {
        revertReason = e.results[e.hashes[0]].reason
    } 
    return revertReason
}

module.exports = {
    provider,
    wallet,
    exitWallet,
    deployTestContract,
    getCallRevertReason
}