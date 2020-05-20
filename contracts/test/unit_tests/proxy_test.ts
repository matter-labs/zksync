const { expect } = require("chai");
const { deployContract } = require("ethereum-waffle");
const { wallet, wallet1, wallet2, deployTestContract, getCallRevertReason } = require("./common");

import {Contract, ethers} from "ethers";
import {AddressZero} from "ethers/constants";

describe("Proxy unit tests", function() {
    this.timeout(50000);

    let proxyTestContract;
    let proxyDummyInterface;
    let DummyFirst;
    before(async () => {
        DummyFirst = await deployTestContract("../../build/DummyFirst");
        proxyTestContract = await deployContract(wallet, require("../../build/Proxy"), [DummyFirst.address, [1, 2]], {
            gasLimit: 6000000,
        });
        proxyDummyInterface = new Contract(proxyTestContract.address, require("../../build/DummyTarget").interface, wallet);

        // check delegatecall
        expect(await proxyDummyInterface.get_DUMMY_INDEX())
            .to.equal(1);
    });

    it("checking that requireMaster calls present", async () => {
        const testContract_with_wallet2_signer = await proxyTestContract.connect(wallet2);
        expect((await getCallRevertReason( () => testContract_with_wallet2_signer.upgradeTarget(AddressZero, []) )).revertReason).equal("oro11");
        expect((await getCallRevertReason( () => testContract_with_wallet2_signer.upgradeNoticePeriodStarted() )).revertReason).equal("oro11");
        expect((await getCallRevertReason( () => testContract_with_wallet2_signer.upgradePreparationStarted() )).revertReason).equal("oro11");
        expect((await getCallRevertReason( () => testContract_with_wallet2_signer.upgradeCanceled() )).revertReason).equal("oro11");
        expect((await getCallRevertReason( () => testContract_with_wallet2_signer.upgradeFinishes() )).revertReason).equal("oro11");
    });

    it("checking Proxy reverts", async () => {
        expect((await getCallRevertReason( () => proxyTestContract.initialize([]) )).revertReason).equal("ini11");
        expect((await getCallRevertReason( () => proxyTestContract.upgradeTarget(proxyTestContract.address, []) )).revertReason).equal("ufu11");
    });

});
