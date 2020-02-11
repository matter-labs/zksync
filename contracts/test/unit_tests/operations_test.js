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
        let r = await testContract.testDeposit()
        console.log(r)
        //expect(r).equal("0x01020311121314")
    });

});
