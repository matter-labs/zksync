import { ethers } from "ethers";

import { expect, use } from "chai";
import { createMockProvider, getWallets, solidity, deployContract } from "ethereum-waffle";
import { bigNumberify, parseEther, hexlify, formatEther } from "ethers/utils";

// For: geth
// const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
// const wallet: any = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);

// For: ganache
const provider = createMockProvider();
const [wallet]  = getWallets(provider);

use(solidity);

export async function deployBytesTestContract() {
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

    let bytesTestContract: any;
    beforeEach(async () => {
        console.log("---\n")
        bytesTestContract = await deployBytesTestContract()
    });

    it("should bla-bla", async () => {
        console.log("\n - Bytes test started");

        let r = await bytesTestContract.testConcat("0x010203", "0x11121314", {});
        expect(r).equal("0x01020311121314");

        console.log(" + Bytes test passed")
    });

});
