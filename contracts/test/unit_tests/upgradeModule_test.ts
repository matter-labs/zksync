import {AddressZero} from "ethers/constants";

const { expect } = require("chai")
const { deployContract } = require("ethereum-waffle");
const { provider, wallet, wallet1, wallet2, deployTestContract, getCallRevertReason } = require("./common")

const { performance } = require('perf_hooks');

// some random constants for checking write and read from storage
const bytes = [133, 174, 97, 255]

import {Contract, ethers} from "ethers";

describe("UpgradeModule unit tests", function () {
    this.timeout(50000);

    let upgradeModuleContract
    let proxyTestContract
    let proxyDummyInterface
    let DummyFirst
    let DummySecond
    before(async () => {
        proxyTestContract = await deployTestContract('../../build/Proxy')
        proxyDummyInterface = new Contract(proxyTestContract.address, require('../../build/DummyTarget').interface, wallet);
        DummyFirst = await deployTestContract('../../build/DummyFirst')
        DummySecond = await deployTestContract('../../build/DummySecond')
        await proxyTestContract.initializeTarget(DummyFirst.address, [bytes[0], bytes[1]]);
        upgradeModuleContract = await deployContract(wallet, require('../../build/UpgradeModuleTest'), [proxyTestContract.address], {
            gasLimit: 6000000,
        })
        proxyTestContract.transferMastership(upgradeModuleContract.address);
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
        let upgradeModuleContract_with_wallet2_signer = await upgradeModuleContract.connect(wallet2);
        expect((await getCallRevertReason( () => upgradeModuleContract_with_wallet2_signer.upgradeProxy(AddressZero, AddressZero) )).revertReason).equal("oro11")
        expect((await getCallRevertReason( () => upgradeModuleContract_with_wallet2_signer.cancelProxyUpgrade(AddressZero) )).revertReason).equal("oro11")
        expect((await getCallRevertReason( () => upgradeModuleContract_with_wallet2_signer.finishProxyUpgrade(AddressZero, []) )).revertReason).equal("oro11")
    });

    it("check UpgradeModule reverts; activate and cancel upgrade", async () => {
        expect((await getCallRevertReason( () => upgradeModuleContract.cancelProxyUpgrade(proxyTestContract.address) )).revertReason).equal("umc11")
        expect((await getCallRevertReason( () => upgradeModuleContract.activeFinalizeStatusOfUpgrade(proxyTestContract.address) )).revertReason).equal("uaf11")
        expect((await getCallRevertReason( () => upgradeModuleContract.finishProxyUpgrade(proxyTestContract.address, []) )).revertReason).equal("umf11")

        await expect(upgradeModuleContract.upgradeProxy(proxyTestContract.address, DummySecond.address))
            .to.emit(upgradeModuleContract, 'UpgradeModeActivated')
            .withArgs(proxyTestContract.address, 0)
        expect((await getCallRevertReason( () => upgradeModuleContract.upgradeProxy(proxyTestContract.address, DummySecond.address) )).revertReason).equal("upa11")
        await expect(upgradeModuleContract.cancelProxyUpgrade(proxyTestContract.address))
            .to.emit(upgradeModuleContract, 'UpgradeCanceled')
            .withArgs(proxyTestContract.address, 0)
    });

    it("checking that the upgrade works correctly", async () => {
        let start_time = performance.now();

        // activate
        await expect(upgradeModuleContract.upgradeProxy(proxyTestContract.address, DummySecond.address))
            .to.emit(upgradeModuleContract, 'UpgradeModeActivated')
            .withArgs(proxyTestContract.address, 0)

        let activated_time = performance.now();

        // wait and activate finalize status
        let all_time_in_sec = parseInt(await upgradeModuleContract.get_WAIT_UPGRADE_MODE_PERIOD());
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
                await upgradeModuleContract.activeFinalizeStatusOfUpgrade(proxyTestContract.address);
            } else {
                await expect(upgradeModuleContract.activeFinalizeStatusOfUpgrade(proxyTestContract.address))
                    .to.emit(upgradeModuleContract, 'UpgradeModeFinalizeStatusActivated')
                    .withArgs(proxyTestContract.address, 0)
            }
        }

        // finish upgrade without verifying priority operations
        expect((await getCallRevertReason( () => upgradeModuleContract.finishProxyUpgrade(proxyTestContract.address, []) )).revertReason).equal("umf13")
        // finish upgrade
        await proxyDummyInterface.verifyPriorityOperation();
        await expect(upgradeModuleContract.finishProxyUpgrade(proxyTestContract.address, [bytes[2], bytes[3]]))
            .to.emit(upgradeModuleContract, 'UpgradeCompleted')
            .withArgs(proxyTestContract.address, 0, DummySecond.address)

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
        await expect(upgradeModuleContract.upgradeProxy(proxyTestContract.address, DummyFirst.address))
            .to.emit(upgradeModuleContract, 'UpgradeModeActivated')
            .withArgs(proxyTestContract.address, 1);
        await expect(upgradeModuleContract.cancelProxyUpgrade(proxyTestContract.address))
            .to.emit(upgradeModuleContract, 'UpgradeCanceled')
            .withArgs(proxyTestContract.address, 1);
    });

});
