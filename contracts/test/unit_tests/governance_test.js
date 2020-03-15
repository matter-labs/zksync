const { expect } = require("chai")
const { wallet, deployProxyContract, getCallRevertReason } = require("./common")

describe("Governance unit tests", function () {
    this.timeout(50000);

    let testContract
    before(async () => {
        let governanceAddressDeployed;
        [testContract, governanceAddressDeployed] = await deployProxyContract(
            wallet,
            require('../../build/Proxy'),
            require('../../build/GovernanceTest'),
            ["address"],
            [wallet.address],
        );
    });

    it("checking correctness of using MAX_AMOUNT_OF_REGISTERED_TOKENS constant", async () => {
        let MAX_AMOUNT_OF_REGISTERED_TOKENS = await testContract.get_MAX_AMOUNT_OF_REGISTERED_TOKENS();
        for (let step = 1; step <= MAX_AMOUNT_OF_REGISTERED_TOKENS + 1; step++) {
            let {revertReason} = await getCallRevertReason( () => testContract.addToken("0x" + step.toString().padStart(40, '0')) )
            if (step != MAX_AMOUNT_OF_REGISTERED_TOKENS + 1) {
                expect(revertReason).equal("VM did not revert")
            }
            else{
                expect(revertReason).not.equal("VM did not revert")
            }
        }
    });

});
