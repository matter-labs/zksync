import * as hardhat from 'hardhat';
import { expect } from 'chai';
import { getCallRevertReason } from './common';
import {
    AdditionalZkSyncCutNoticePeriodUnitTestFactory,
    AdditionalZkSyncCutNoticePeriodUnitTest,
    DummyUpgradeGatekeeper,
    DummyUpgradeGatekeeperFactory
} from '../typechain';
import * as fs from 'fs';
import * as path from 'path';
import { ethers } from 'hardhat';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

const SECURITY_COUNCIL_MEMBERS_NUMBER = parseInt(hardhat.config.solpp.defs.SECURITY_COUNCIL_MEMBERS_NUMBER);
const SECURITY_COUNCIL_MEMBERS = hardhat.config.solpp.defs.SECURITY_COUNCIL_MEMBERS.split(',');
const SECURITY_COUNCIL_THRESHOLD = parseInt(hardhat.config.solpp.defs.SECURITY_COUNCIL_THRESHOLD);
const UPGRADE_GATEKEEPER_ADDRESS = hardhat.config.solpp.defs.UPGRADE_GATEKEEPER_ADDRESS;

async function getUpgradeTargetsHash(gatekeeper): Promise<string> {
    const targets = await Promise.all([
        gatekeeper.nextTargets(0),
        gatekeeper.nextTargets(1),
        gatekeeper.nextTargets(2)
    ]);
    return ethers.utils.solidityKeccak256(['address', 'address', 'address'], targets);
}

async function signApproveCutUpgradeNoticePeriod(gatekeeper, signer): Promise<string> {
    const targetsHash = await getUpgradeTargetsHash(gatekeeper);
    const signature = await signer.signMessage(`Approved new ZkSync's target contracts hash\n${targetsHash}`);

    return signature;
}

describe('Instant upgrade with security council members', function () {
    let securityCouncilMembers;
    let zkSyncTarget: AdditionalZkSyncCutNoticePeriodUnitTest;
    let upgradeGatekeeper: DummyUpgradeGatekeeper;

    before(async () => {
        // Address values of security council are hardcoded in contracts.
        // Get accounts that can sign messages and check that they correspond to addresses that are hardcoded in the tests contract.
        securityCouncilMembers = [];
        for (let i = 0; i < SECURITY_COUNCIL_MEMBERS_NUMBER; ++i) {
            const account = await hardhat.ethers.Wallet.fromMnemonic(
                ethTestConfig.test_mnemonic,
                "m/44'/60'/0'/0/" + i
            ).connect(hardhat.ethers.provider);
            securityCouncilMembers.push(account);
            await hardhat.network.provider.send('hardhat_setBalance', [account.address, '0xfffffffffffffffff']);

            expect(account.address).to.eq(SECURITY_COUNCIL_MEMBERS[i]);
        }

        const zkSyncFactory = await hardhat.ethers.getContractFactory('AdditionalZkSyncCutNoticePeriodUnitTest');
        const zkSyncContract = await zkSyncFactory.deploy();
        zkSyncTarget = AdditionalZkSyncCutNoticePeriodUnitTestFactory.connect(
            zkSyncContract.address,
            zkSyncContract.signer
        );
        await zkSyncTarget.disableUpgrade();

        // The address of `upgradeGatekeeper` is hardcoded as well, so we can't deploy a new contract
        // so we set the bytecode to the address that is already hardcoded in the `additionalZkSync`
        const upgradeGatekeeperArtifacts = await hardhat.artifacts.readArtifact('DummyUpgradeGatekeeper');
        await hardhat.network.provider.send('hardhat_setCode', [
            UPGRADE_GATEKEEPER_ADDRESS,
            upgradeGatekeeperArtifacts.deployedBytecode
        ]);
        upgradeGatekeeper = DummyUpgradeGatekeeperFactory.connect(UPGRADE_GATEKEEPER_ADDRESS, zkSyncContract.signer);
    });

    it('should fail to speed up upgrade before upgrade started', async () => {
        const { revertReason } = await getCallRevertReason(() =>
            zkSyncTarget.cutUpgradeNoticePeriod(ethers.constants.HashZero)
        );
        expect(revertReason).to.eq('p1');
    });

    it('should fail to speed up upgrade with signatures before upgrade started', async () => {
        const { revertReason } = await getCallRevertReason(() => zkSyncTarget.cutUpgradeNoticePeriodBySignature([]));
        expect(revertReason).to.eq('p2');
    });

    context('cut upgrade notice period through `cutUpgradeNoticePeriod`', function () {
        before(async () => {
            const targets = [ethers.constants.AddressZero, ethers.constants.AddressZero, zkSyncTarget.address];
            await upgradeGatekeeper.setNextTargets(targets);
        });

        beforeEach(async () => {
            await zkSyncTarget.enableUpgradeFromScratch();
        });

        it('should NOT cut upgrade notice period without permission', async () => {
            const targetsHash = await getUpgradeTargetsHash(upgradeGatekeeper);
            const tx = await zkSyncTarget.cutUpgradeNoticePeriod(targetsHash);
            expect(tx).to.not.emit(zkSyncTarget, 'NoticePeriodChange');
            expect(tx).to.not.emit(zkSyncTarget, 'ApproveCutUpgradeNoticePeriod');
        });

        it('should NOT cut upgrade notice period with incorrect targets hash', async () => {
            const { revertReason } = await getCallRevertReason(() =>
                zkSyncTarget.cutUpgradeNoticePeriod(ethers.constants.HashZero)
            );
            expect(revertReason).to.eq('p3');
        });

        it('cut upgrade notice period', async () => {
            const targetsHash = await getUpgradeTargetsHash(upgradeGatekeeper);
            const noticePeriodBefore = await zkSyncTarget.getApprovedUpgradeNoticePeriod();
            for (let i = 0; i < SECURITY_COUNCIL_MEMBERS_NUMBER; ++i) {
                const tx = await zkSyncTarget.connect(securityCouncilMembers[i]).cutUpgradeNoticePeriod(targetsHash);
                expect(tx).to.emit(zkSyncTarget, 'ApproveCutUpgradeNoticePeriod');
                if (i == SECURITY_COUNCIL_THRESHOLD - 1) {
                    expect(tx).to.emit(zkSyncTarget, 'NoticePeriodChange');
                } else {
                    expect(tx).to.not.emit(zkSyncTarget, 'NoticePeriodChange');
                }
            }
            const noticePeriodAfter = await zkSyncTarget.getApprovedUpgradeNoticePeriod();

            expect(noticePeriodBefore).to.not.eq(0);
            expect(noticePeriodAfter).to.eq(0);
        });
    });

    context('cut upgrade notice period through `cutUpgradeNoticePeriodBySignature`', function () {
        before(async () => {
            const targets = [ethers.constants.AddressZero, ethers.constants.AddressZero, zkSyncTarget.address];
            await upgradeGatekeeper.setNextTargets(targets);
        });

        beforeEach(async () => {
            await zkSyncTarget.enableUpgradeFromScratch();
        });

        it('should NOT cut upgrade notice period without permission', async () => {
            const signature = await signApproveCutUpgradeNoticePeriod(upgradeGatekeeper, zkSyncTarget.signer);
            const tx = await zkSyncTarget.cutUpgradeNoticePeriodBySignature([signature]);
            expect(tx).to.not.emit(zkSyncTarget, 'NoticePeriodChange');
            expect(tx).to.not.emit(zkSyncTarget, 'ApproveCutUpgradeNoticePeriod');
        });

        it('cut upgrade notice period', async () => {
            const noticePeriodBefore = await zkSyncTarget.getApprovedUpgradeNoticePeriod();
            const signatures = [];
            for (let i = 0; i < SECURITY_COUNCIL_THRESHOLD; ++i) {
                const signature = await signApproveCutUpgradeNoticePeriod(upgradeGatekeeper, securityCouncilMembers[i]);
                signatures.push(signature);
            }
            const tx = await zkSyncTarget.cutUpgradeNoticePeriodBySignature(signatures);
            expect(tx).to.emit(zkSyncTarget, 'ApproveCutUpgradeNoticePeriod');
            expect(tx).to.emit(zkSyncTarget, 'NoticePeriodChange');
            const noticePeriodAfter = await zkSyncTarget.getApprovedUpgradeNoticePeriod();

            expect(noticePeriodBefore).to.not.eq(0);
            expect(noticePeriodAfter).to.eq(0);
        });
    });
});
