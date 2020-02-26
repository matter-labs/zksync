const ethers = require("ethers")
const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { provider, wallet, wallet2, deployTestContract, getCallRevertReason, SKIP_TEST } = require("./common")

const { performance } = require('perf_hooks');

const proxyTestContractCode = require('../../build/ProxyTest');

// some random constants for checking write and read from storage
const bytes = [133, 174, 97, 255]

describe("Proxy unit test", function () {
    this.timeout(50000);

    let proxyTestContract
    let proxyDummyInterface
    let upgradeModeTestContract
    let DummyFirst
    let DummySecond
    before(async () => {
        proxyTestContract = await deployTestContract('../../build/ProxyTest')
        proxyDummyInterface = new ethers.Contract(proxyTestContract.address, require('../../build/DummyTarget').interface, wallet);
        upgradeModeTestContract = new ethers.Contract(proxyTestContract.getUpgradeModeTestAddress(), require('../../build/UpgradeModeTest').interface, wallet);
        DummyFirst = await deployTestContract('../../build/DummyFirst')
        DummySecond = await deployTestContract('../../build/DummySecond')
        await proxyTestContract.initialize(DummyFirst.address, [bytes[0], bytes[1]]);
    });

    it("checking Proxy creation", async () => {
        // check version
        expect(await proxyTestContract.getVersion()).to.equal(1)

        // check target storage
        expect((await provider.getStorageAt(proxyTestContract.address, ethers.utils.id("target"))).toLowerCase())
            .equal(DummyFirst.address.toLowerCase());
        expect((await proxyTestContract.getTarget()).toLowerCase())
            .equal(DummyFirst.address.toLowerCase());

        // check dummy index
        expect(await proxyDummyInterface.get_DUMMY_INDEX())
            .to.equal(1);

        // check initial storage
        expect((await provider.getStorageAt(proxyTestContract.address, 0)).toLowerCase())
            .equal((await proxyTestContract.getUpgradeModeTestAddress()).toLowerCase());
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 1)))
            .to.equal(bytes[0]);
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 2)))
            .to.equal(bytes[1]);
    });

    it("checking that requireMaster calls present", async () => {
        let proxyTestContract_with_wallet2_signer = await proxyTestContract.connect(wallet2);
        expect(await getCallRevertReason( () => proxyTestContract_with_wallet2_signer.upgradeTarget(DummySecond.address) )).equal("oro11")
        expect(await getCallRevertReason( () => proxyTestContract_with_wallet2_signer.cancelUpgradeTarget() )).equal("oro11")
        expect(await getCallRevertReason( () => proxyTestContract_with_wallet2_signer.finishTargetUpgrade([]) )).equal("oro11")

        // bonus: check that force cancellation do not have requireMaster call
        expect(await getCallRevertReason( () => proxyTestContract_with_wallet2_signer.forceCancelUpgradeTarget() )).to.not.equal("oro11")
    });

    it("check Proxy reverts", async () => {
        expect(await getCallRevertReason( () => proxyTestContract.initialize(DummyFirst.address, []) )).equal("uin11");
        expect(await getCallRevertReason( () => proxyTestContract.upgradeTarget("0x0000000000000000000000000000000000000000") )).equal("uut11");
        expect(await getCallRevertReason( () => proxyTestContract.upgradeTarget(DummyFirst.address) )).equal("uut12");
    });

    it("check upgrade canceling", async () => {
        // activate and cancel
        await proxyTestContract.upgradeTarget(DummySecond.address);
        await proxyTestContract.cancelUpgradeTarget();
    });

    if (SKIP_TEST) {
        it.skip("checking that the upgrade is done correctly", async () => {});
    }
    else {
        it("checking that the upgrade is done correctly", async () => {
            let start_time = performance.now();

            // activate
            await proxyTestContract.upgradeTarget(DummySecond.address);

            let activated_time = performance.now();

            // wait and finish upgrade
            let all_time_in_sec = parseInt(await upgradeModeTestContract.get_WAIT_UPGRADE_MODE_PERIOD());
            for (let step = 1; step <= 3; step++) {
                if (step != 3) {
                    while ((performance.now() - start_time) < Math.round(all_time_in_sec * 1000.0 * step / 10.0 + 10)) {
                        // wait
                    }
                } else {
                    while ((performance.now() - activated_time) < all_time_in_sec * 1000 + 10) {
                        // wait
                    }
                }

                if (step != 3) {
                    expect(await getCallRevertReason( () => proxyTestContract.finishTargetUpgrade([]))).equal("umf11");
                } else {
                    await proxyTestContract.finishTargetUpgrade([bytes[2], bytes[3]]);
                }
            }

            // check dummy index
            expect(await proxyDummyInterface.get_DUMMY_INDEX())
                .to.equal(2);

            // check updated storage
            expect((await provider.getStorageAt(proxyTestContract.address, 0)).toLowerCase())
                .equal((await proxyTestContract.getUpgradeModeTestAddress()).toLowerCase());
            expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 1)))
                .to.equal(bytes[0]);
            expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 2)))
                .to.equal(bytes[2]);
            expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 3)))
                .to.equal(bytes[3]);
        });
    }

    if (SKIP_TEST) {
        it.skip("checking that force cancellation works correctly", async () => {});
    }
    else {
        it("checking that force cancellation works correctly", async () => {
            expect(await getCallRevertReason( () => proxyTestContract.forceCancelUpgradeTarget())).equal("ufc11");

            let start_time = performance.now();

            // activate
            await proxyTestContract.upgradeTarget(DummyFirst.address);

            let activated_time = performance.now();

            // wait and finish upgrade
            let all_time_in_sec = parseInt(await upgradeModeTestContract.get_MAX_UPGRADE_PERIOD());
            for (let step = 1; step <= 3; step++) {
                if (step != 3) {
                    while ((performance.now() - start_time) < Math.round(all_time_in_sec * 1000.0 * step / 10.0 + 10)) {
                        // wait
                    }
                } else {
                    while ((performance.now() - activated_time) < all_time_in_sec * 1000 + 10) {
                        // wait
                    }
                }

                if (step != 3) {
                    expect(await getCallRevertReason( () => proxyTestContract.forceCancelUpgradeTarget())).equal("ufc12");
                } else {
                    expect(await getCallRevertReason( () => proxyTestContract.finishTargetUpgrade([]))).equal("ufu11");
                    await proxyTestContract.forceCancelUpgradeTarget();
                }
            }

            expect(await upgradeModeTestContract.waitUpgradeModeActive()).to.equal(false)
            // check dummy index
            expect(await proxyDummyInterface.get_DUMMY_INDEX())
                .to.equal(2);
        });
    }

});
