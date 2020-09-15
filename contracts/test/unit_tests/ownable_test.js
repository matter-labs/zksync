const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { wallet1, wallet2, deployTestContract, getCallRevertReason } = require("./common")


describe("Ownable unit tests", function () {
    this.timeout(50000);

    let testContract
    before(async () => {
        testContract = await deployContract(wallet1, require('../../build/Ownable'), [wallet1.address], {
            gasLimit: 6000000,
        })
    });

    it("checking correctness of setting mastership in constructor", async () => {
        expect(await testContract.getMaster()).to.equal(wallet1.address)
    });

    it("checking correctness of transferring mastership to zero address", async () => {
        let {revertReason} = await getCallRevertReason( () => testContract.transferMastership("0x0000000000000000000000000000000000000000", {gasLimit: "300000"}) );
        expect(revertReason).equal("otp11")
    });

    it("checking correctness of transferring mastership", async () => {
        /// transfer mastership to wallet2
        await testContract.transferMastership(wallet2.address);
        expect(await testContract.getMaster()).to.equal(wallet2.address)

        /// try to transfer mastership to wallet1 by wallet1 call
        let {revertReason} = await getCallRevertReason( () => testContract.transferMastership(wallet1.address, {gasLimit: "300000"}) );
        expect(revertReason).equal("oro11")

        /// transfer mastership back to wallet1
        let testContract_with_wallet2_signer = await testContract.connect(wallet2);
        await testContract_with_wallet2_signer.transferMastership(wallet1.address);
        expect(await testContract.getMaster()).to.equal(wallet1.address)
    });

});
