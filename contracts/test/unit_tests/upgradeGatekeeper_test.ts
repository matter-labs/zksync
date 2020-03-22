import {AddressZero} from "ethers/constants";

const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { provider, wallet, wallet1, wallet2, deployTestContract, getCallRevertReason } = require("./common")

const { performance } = require('perf_hooks');

// some random constants for checking write and read from storage
const bytes = [133, 174, 97, 255]

import {Contract, ethers} from "ethers";

describe("UpgradeGatekeeper unit tests", function () {
    this.timeout(50000);

    let UpgradeGatekeeperContract
    let proxyTestContract
    let proxyDummyInterface
    let DummyFirst
    let DummySecond
    before(async () => {
        DummyFirst = await deployTestContract('../../build/DummyFirst')
        DummySecond = await deployTestContract('../../build/DummySecond')
        proxyTestContract = await deployContract(wallet, require('../../build/Proxy'), [DummyFirst.address, [bytes[0], bytes[1]]], {
            gasLimit: 6000000,
        })
        proxyDummyInterface = new Contract(proxyTestContract.address, require('../../build/DummyTarget').interface, wallet);
        UpgradeGatekeeperContract = await deployContract(wallet, require('../../build/UpgradeGatekeeperTest'), [proxyTestContract.address], {
            gasLimit: 6000000,
        })
        proxyTestContract.transferMastership(UpgradeGatekeeperContract.address);
    });

    it("check initial dummy index and storage", async () => {
        expect(await proxyDummyInterface.get_DUMMY_INDEX())
            .to.equal(1);

        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 1)))
            .to.equal(bytes[0]);
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 2)))
            .to.equal(bytes[1]);
    });

    it("checking that requireMaster calls present", async () => {
        let UpgradeGatekeeperContract_with_wallet2_signer = await UpgradeGatekeeperContract.connect(wallet2);
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract_with_wallet2_signer.upgradeProxy(AddressZero, AddressZero) )).revertReason).equal("oro11")
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract_with_wallet2_signer.cancelProxyUpgrade(AddressZero) )).revertReason).equal("oro11")
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract_with_wallet2_signer.finishProxyUpgrade(AddressZero, []) )).revertReason).equal("oro11")
    });

    it("check UpgradeGatekeeper reverts; activate and cancel upgrade", async () => {
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.cancelProxyUpgrade(proxyTestContract.address) )).revertReason).equal("umc11")
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.activateCleaningUpStatusOfUpgrade(proxyTestContract.address) )).revertReason).equal("uaf11")
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.finishProxyUpgrade(proxyTestContract.address, []) )).revertReason).equal("umf11")

        await expect(UpgradeGatekeeperContract.upgradeProxy(proxyTestContract.address, DummySecond.address))
            .to.emit(UpgradeGatekeeperContract, 'UpgradeModeActivated')
            .withArgs(proxyTestContract.address, 0)
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.upgradeProxy(proxyTestContract.address, DummySecond.address) )).revertReason).equal("upa11")
        await expect(UpgradeGatekeeperContract.cancelProxyUpgrade(proxyTestContract.address))
            .to.emit(UpgradeGatekeeperContract, 'UpgradeCanceled')
            .withArgs(proxyTestContract.address, 0)
    });

    it("checking that the upgrade works correctly", async () => {
        let start_time = performance.now();

        // activate
        await expect(UpgradeGatekeeperContract.upgradeProxy(proxyTestContract.address, DummySecond.address))
            .to.emit(UpgradeGatekeeperContract, 'UpgradeModeActivated')
            .withArgs(proxyTestContract.address, 0)

        let activated_time = performance.now();

        // wait and activate cleaning up status
        let all_time_in_sec = parseInt(await UpgradeGatekeeperContract.get_NOTICE_PERIOD());
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
                await UpgradeGatekeeperContract.activateCleaningUpStatusOfUpgrade(proxyTestContract.address);
            } else {
                await expect(UpgradeGatekeeperContract.activateCleaningUpStatusOfUpgrade(proxyTestContract.address))
                    .to.emit(UpgradeGatekeeperContract, 'UpgradeModeCleaningUpStatusActivated')
                    .withArgs(proxyTestContract.address, 0)
            }
        }

        // finish upgrade without verifying priority operations
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.finishProxyUpgrade(proxyTestContract.address, []) )).revertReason).equal("umf13")
        // finish upgrade
        await proxyDummyInterface.verifyPriorityOperation();
        await expect(UpgradeGatekeeperContract.finishProxyUpgrade(proxyTestContract.address, [bytes[2], bytes[3]]))
            .to.emit(UpgradeGatekeeperContract, 'UpgradeCompleted')
            .withArgs(proxyTestContract.address, 0, DummySecond.address)

        await expect(await proxyTestContract.getTarget())
            .to.equal(DummySecond.address);

        // check dummy index and updated storage
        expect(await proxyDummyInterface.get_DUMMY_INDEX())
            .to.equal(2);

        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 1)))
            .to.equal(bytes[0]);
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 2)))
            .to.equal(bytes[2]);
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 3)))
            .to.equal(bytes[3]);

        // one more activate and cancel with version equal to 1
        await expect(UpgradeGatekeeperContract.upgradeProxy(proxyTestContract.address, DummyFirst.address))
            .to.emit(UpgradeGatekeeperContract, 'UpgradeModeActivated')
            .withArgs(proxyTestContract.address, 1);
        await expect(UpgradeGatekeeperContract.cancelProxyUpgrade(proxyTestContract.address))
            .to.emit(UpgradeGatekeeperContract, 'UpgradeCanceled')
            .withArgs(proxyTestContract.address, 1);
    });

});
