import { constants } from 'ethers';
const { expect } = require('chai');
const { getCallRevertReason } = require('./common');
const { performance } = require('perf_hooks');
const hardhat = require('hardhat');

// some random constants for checking write and read from storage
const bytes = [133, 174, 97, 255];

describe('UpgradeGatekeeper unit tests', function () {
    this.timeout(50000);

    let provider;
    let wallet;
    let proxyTestContract, proxyDummyInterface;
    let dummyFirst, dummySecond;
    let upgradeGatekeeperContract;
    before(async () => {
        provider = hardhat.ethers.provider;
        const wallets = await hardhat.ethers.getSigners();
        // Get some wallet different from than the default one.
        wallet = wallets[1];

        const dummy1Factory = await hardhat.ethers.getContractFactory('DummyFirst');
        dummyFirst = await dummy1Factory.deploy();
        const dummy2Factory = await hardhat.ethers.getContractFactory('DummySecond');
        dummySecond = await dummy2Factory.deploy();

        const proxyFactory = await hardhat.ethers.getContractFactory('Proxy');
        proxyTestContract = await proxyFactory.deploy(dummyFirst.address, [bytes[0], bytes[1]]);

        proxyDummyInterface = await hardhat.ethers.getContractAt('DummyTarget', proxyTestContract.address);

        const upgradeGatekeeperFactory = await hardhat.ethers.getContractFactory('UpgradeGatekeeper');
        upgradeGatekeeperContract = await upgradeGatekeeperFactory.deploy(proxyTestContract.address);

        await proxyTestContract.transferMastership(upgradeGatekeeperContract.address);

        await expect(upgradeGatekeeperContract.addUpgradeable(proxyTestContract.address)).to.emit(
            upgradeGatekeeperContract,
            'NewUpgradable'
        );

        // check initial dummy index and storage
        expect(await proxyDummyInterface.getDummyIndex()).to.equal(1);

        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 1))).to.equal(bytes[0]);
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 2))).to.equal(bytes[1]);
    });

    it('checking that requireMaster calls present', async () => {
        const UpgradeGatekeeperContract_with_wallet2_signer = await upgradeGatekeeperContract.connect(wallet);
        expect(
            (
                await getCallRevertReason(() =>
                    UpgradeGatekeeperContract_with_wallet2_signer.addUpgradeable(constants.AddressZero)
                )
            ).revertReason
        ).equal('1c');
        expect(
            (await getCallRevertReason(() => UpgradeGatekeeperContract_with_wallet2_signer.startUpgrade([])))
                .revertReason
        ).equal('1c');
        expect(
            (await getCallRevertReason(() => UpgradeGatekeeperContract_with_wallet2_signer.cancelUpgrade()))
                .revertReason
        ).equal('1c');
        expect(
            (await getCallRevertReason(() => UpgradeGatekeeperContract_with_wallet2_signer.finishUpgrade([])))
                .revertReason
        ).equal('1c');
    });

    it('checking UpgradeGatekeeper reverts; activation and cancellation upgrade', async () => {
        expect((await getCallRevertReason(() => upgradeGatekeeperContract.cancelUpgrade())).revertReason).equal(
            'cpu11'
        );
        expect((await getCallRevertReason(() => upgradeGatekeeperContract.startPreparation())).revertReason).equal(
            'ugp11'
        );
        expect((await getCallRevertReason(() => upgradeGatekeeperContract.finishUpgrade([]))).revertReason).equal(
            'fpu11'
        );

        expect((await getCallRevertReason(() => upgradeGatekeeperContract.startUpgrade([]))).revertReason).equal(
            'spu12'
        );
        await expect(upgradeGatekeeperContract.startUpgrade([dummySecond.address])).to.emit(
            upgradeGatekeeperContract,
            'NoticePeriodStart'
        );
        expect((await getCallRevertReason(() => upgradeGatekeeperContract.startUpgrade([]))).revertReason).equal(
            'spu11'
        );
        await expect(upgradeGatekeeperContract.cancelUpgrade()).to.emit(upgradeGatekeeperContract, 'UpgradeCancel');
    });

    it('checking that the upgrade works correctly', async () => {
        const start_time = performance.now();

        // activate
        await expect(upgradeGatekeeperContract.startUpgrade([dummySecond.address])).to.emit(
            upgradeGatekeeperContract,
            'NoticePeriodStart'
        );

        const activated_time = performance.now();

        // wait and activate preparation status
        const notice_period = parseInt(await dummyFirst.getNoticePeriod());
        for (let step = 1; step <= 3; step++) {
            if (step != 3) {
                while (performance.now() - start_time < Math.round((notice_period * 1000.0 * step) / 10.0 + 10)) {
                    // wait
                }
            } else {
                while (performance.now() - activated_time < notice_period * 1000 + 10) {
                    // wait
                }
            }

            if (step !== 3) {
                await upgradeGatekeeperContract.startPreparation();
            } else {
                await expect(upgradeGatekeeperContract.startPreparation()).to.emit(
                    upgradeGatekeeperContract,
                    'PreparationStart'
                );
            }
        }

        expect((await getCallRevertReason(() => upgradeGatekeeperContract.finishUpgrade([]))).revertReason).equal(
            'fpu12'
        );
        // finish upgrade without verifying priority operations
        expect(
            (await getCallRevertReason(() => upgradeGatekeeperContract.finishUpgrade([[bytes[2], bytes[3]]])))
                .revertReason
        ).equal('fpu13');
        // finish upgrade
        await proxyDummyInterface.verifyPriorityOperation();
        await expect(upgradeGatekeeperContract.finishUpgrade([[bytes[2], bytes[3]]])).to.emit(
            upgradeGatekeeperContract,
            'UpgradeComplete'
        );

        await expect(await proxyTestContract.getTarget()).to.equal(dummySecond.address);

        // check dummy index and updated storage
        expect(await proxyDummyInterface.getDummyIndex()).to.equal(2);

        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 1))).to.equal(bytes[0]);
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 2))).to.equal(bytes[2]);
        expect(parseInt(await provider.getStorageAt(proxyTestContract.address, 3))).to.equal(bytes[3]);
    });
});
