const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { wallet, wallet1, wallet2, deployTestContract, getCallRevertReason } = require("./common")

import {Contract, ethers} from "ethers";
import {AddressZero} from "ethers/constants";

describe("Proxy unit tests", function () {
    this.timeout(50000);

    let proxyTestContract
    let proxyDummyInterface
    let DummyFirst
    before(async () => {
        proxyTestContract = await deployTestContract('../../build/ProxyTest')
        proxyDummyInterface = new Contract(proxyTestContract.address, require('../../build/DummyTarget').interface, wallet);
        DummyFirst = await deployTestContract('../../build/DummyFirst')
        await proxyTestContract.initializeTarget(DummyFirst.address, [1, 2]);
    });

    it("checking that requireMaster calls present", async () => {
        let testContract_with_wallet2_signer = await proxyTestContract.connect(wallet2);
        expect((await getCallRevertReason( () => testContract_with_wallet2_signer.initializeTarget(AddressZero, []) )).revertReason).equal("oro11")
        expect((await getCallRevertReason( () => testContract_with_wallet2_signer.upgradeTarget(AddressZero) )).revertReason).equal("oro11")
        expect((await getCallRevertReason( () => testContract_with_wallet2_signer.finishTargetUpgrade(AddressZero, []) )).revertReason).equal("oro11")
    });

    it("check Proxy reverts", async () => {
        expect((await getCallRevertReason( () => proxyTestContract.initialize([]) )).revertReason).equal("ini11")
        expect((await getCallRevertReason( () => proxyTestContract.initializeTarget(proxyTestContract.address, []) )).revertReason).equal("uin11")
        expect((await getCallRevertReason( () => proxyTestContract.upgradeTarget(AddressZero) )).revertReason).equal("uut11")
        expect((await getCallRevertReason( () => proxyTestContract.upgradeTarget(DummyFirst.address) )).revertReason).equal("uut12")
        expect((await getCallRevertReason( () => proxyTestContract.finishTargetUpgrade(proxyTestContract.address, []) )).revertReason).equal("ufu11")
    });

});
