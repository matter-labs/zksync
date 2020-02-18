const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { wallet, deployTestContract, getCallRevertReason } = require("./common")


describe("Ownable unit test", function () {
    this.timeout(50000);

    let testContract
    before(async () => {
        testContract = await deployTestContract('../../build/OwnableTest')
    });

    it("checking correctness of setting mastership in constructor", async () => {
        expect(await testContract.getMaster()).to.equal(wallet.address)
    });

});
