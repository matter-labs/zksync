const { expect } = require("chai")
const { bigNumberify } = require("ethers/utils");
const { provider, wallet, deployTestContract, getCallRevertReason } = require("./common")

describe("Bytes unit test", function () {
    this.timeout(50000);

    let bytesTestContract
    before(async () => {
        bytesTestContract = await deployTestContract('../../build/BytesTest')
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

    // types

    it("should convert uint24", async () => {
        const x = 0x010203;
        let r = await bytesTestContract.testUInt24(x)
        expect(x).equal(r.r)
        expect(r.offset).equal(3)
    });

});
