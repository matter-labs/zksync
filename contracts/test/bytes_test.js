const ethers = require("ethers")
const { expect, use } = require("chai")
const { createMockProvider, getWallets, solidity, deployContract } = require("ethereum-waffle");
const { bigNumberify, parseEther, hexlify, formatEther } = require("ethers/utils");

// For: geth
// const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
// const wallet: any = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

// For: ganache
const provider = createMockProvider();
const [wallet]  = getWallets(provider);

use(solidity);

async function deployBytesTestContract() {
    try {
        return await deployContract(wallet, require('../build/BytesTest'), [], {
            gasLimit: 2000000,
        })
    } catch (err) {
        console.log("BytesTest deploy error:" + err)
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

describe("Bytes unit test", function () {
    this.timeout(50000);

    let bytesTestContract
    before(async () => {
        console.log("---\n")
        bytesTestContract = await deployBytesTestContract()
    });

    // concat

    it("should concatenate bytes", async () => {
        let r = await bytesTestContract.concat("0x010203", "0x11121314")
        expect(r).equal("0x01020311121314")
    });

    // read 

    it("should read bytes", async () => {
        let r = await bytesTestContract.read("0x0102030405060708", 4, 2)
        expect(r.data).equal("0x0506")
        expect(r.new_offset).equal(bigNumberify(6))
    });

    it("should fail to read bytes beyond range", async () => {
        let revertReason = await getCallRevertReason( () => bytesTestContract.read("0x0102030405060708", 8, 2) )
        expect(revertReason).equal("bse11")
    });

    it("should fail to read too many bytes", async () => {
        let revertReason = await getCallRevertReason( () => bytesTestContract.read("0x0102030405060708", 4, 5) )
        expect(revertReason).equal("bse11")
    });

});
