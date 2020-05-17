import {AddressZero} from "ethers/constants";

const { expect } = require("chai");
const { deployContract } = require("ethereum-waffle");
const { provider, wallet, wallet1, wallet2, deployTestContract, getCallRevertReason } = require("./common");

const { performance } = require("perf_hooks");

// some random constants for checking write and read from storage
const bytes = [133, 174, 97, 255];

import {Contract, ethers} from "ethers";

describe("UpgradeGatekeeper unit tests", function() {
    this.timeout(50000);

    let UpgradeGatekeeperContract;
    let proxyTestContract;
    let proxyDummyInterface;
    let DummyFirst;
    let DummySecond;
    before(async () => {
        DummyFirst = await deployTestContract("../../build/DummyFirst");
        DummySecond = await deployTestContract("../../build/DummySecond");
        proxyTestContract = await deployContract(wallet, require("../../build/Proxy"), [DummyFirst.address, [bytes[0], bytes[1]]], {
            gasLimit: 6000000,
        });
        proxyDummyInterface = new Contract(proxyTestContract.address, require("../../build/DummyTarget").interface, wallet);
        UpgradeGatekeeperContract = await deployContract(wallet, require("../../build/UpgradeGatekeeperTest"), [proxyTestContract.address], {
            gasLimit: 6000000,
        });
        await proxyTestContract.transferMastership(UpgradeGatekeeperContract.address);

        await UpgradeGatekeeperContract.addUpgradeable(proxyTestContract.address);

        // check initial dummy index and storage
        expect(await proxyDummyInterface.get_DUMMY_INDEX())
            .to.equal(1);

        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 1)))
            .to.equal(bytes[0]);
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 2)))
            .to.equal(bytes[1]);
    });

    it("checking that requireMaster calls present", async () => {
        const UpgradeGatekeeperContract_with_wallet2_signer = await UpgradeGatekeeperContract.connect(wallet2);
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract_with_wallet2_signer.addUpgradeable(AddressZero) )).revertReason).equal("oro11");
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract_with_wallet2_signer.startUpgrade([]) )).revertReason).equal("oro11");
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract_with_wallet2_signer.cancelUpgrade() )).revertReason).equal("oro11");
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract_with_wallet2_signer.finishUpgrade([], []) )).revertReason).equal("oro11");
    });

    it("checking UpgradeGatekeeper reverts; activation and cancelation upgrade", async () => {
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.cancelUpgrade() )).revertReason).equal("cpu11");
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.startPreparation() )).revertReason).equal("ugp11");
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.finishUpgrade([], []) )).revertReason).equal("fpu11");

        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.startUpgrade([]) )).revertReason).equal("spu12");
        await expect(UpgradeGatekeeperContract.startUpgrade([DummySecond.address]))
            .to.emit(UpgradeGatekeeperContract, "NoticePeriodStart");
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.startUpgrade([]) )).revertReason).equal("spu11");
        await expect(UpgradeGatekeeperContract.cancelUpgrade())
            .to.emit(UpgradeGatekeeperContract, "UpgradeCancel");
    });

    it("checking that the upgrade works correctly", async () => {
        const start_time = performance.now();

        // activate
        await expect(UpgradeGatekeeperContract.startUpgrade([DummySecond.address]))
            .to.emit(UpgradeGatekeeperContract, "NoticePeriodStart");

        const activated_time = performance.now();

        // wait and activate preparation status
        const notice_period = parseInt(await DummyFirst.get_UPGRADE_NOTICE_PERIOD());
        for (let step = 1; step <= 3; step++) {
            if (step != 3) {
                while ((performance.now() - start_time) < Math.round(notice_period * 1000.0 * step / 10.0 + 10)) {
                    // wait
                }
            } else {
                while ((performance.now() - activated_time) < notice_period * 1000 + 10) {
                    // wait
                }
            }

            if (step != 3) {
                await UpgradeGatekeeperContract.startPreparation();
            } else {
                await expect(UpgradeGatekeeperContract.startPreparation())
                    .to.emit(UpgradeGatekeeperContract, "PreparationStart");
            }
        }

        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.finishUpgrade([], []) )).revertReason).equal("fpu12");
        // finish upgrade without verifying priority operations
        expect((await getCallRevertReason( () => UpgradeGatekeeperContract.finishUpgrade([[bytes[2], bytes[3]]]) )).revertReason).equal("fpu13");
        // finish upgrade
        await proxyDummyInterface.verifyPriorityOperation();
        await expect(UpgradeGatekeeperContract.finishUpgrade([[bytes[2], bytes[3]]]))
            .to.emit(UpgradeGatekeeperContract, "UpgradeComplete")
            .withArgs(proxyTestContract.address, DummySecond.address);

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
    });

});
