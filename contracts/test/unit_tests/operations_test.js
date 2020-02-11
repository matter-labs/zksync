const { expect } = require("chai")
const { provider, wallet, deployTestContract, getCallRevertReason } = require("./common")

describe("Operations unit test", function () {
    this.timeout(50000);

    let testContract
    before(async () => {
        console.log("---\n")
        testContract = await deployTestContract('../../build/OperationsTest')
    });

    it("should convert Deposit pubdata", async () => {
        await testContract.testDeposit()
    });

    it("should convert FullExit pubdata", async () => {
        await testContract.testFullExit()
    });

    it("should convert PartialExit pubdata", async () => {
        await testContract.testPartialExit()
    });

});
