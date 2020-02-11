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

describe("Bytes unit test", function () {
    this.timeout(50000);

    let bytesTestContract
    before(async () => {
        console.log("---\n")
        bytesTestContract = await deployBytesTestContract()
    });

    it("should concatenate bytes", async () => {
        let r = await bytesTestContract.concat("0x010203", "0x11121314")
        expect(r).equal("0x01020311121314")
    });

    it("should read bytes", async () => {
        let r = await bytesTestContract.read("0x0102030405060708", 4, 2)
        //expect(r).equal("0x01020311121314");
        console.log(r)
    });

});
