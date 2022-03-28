import * as hardhat from 'hardhat';
import { expect } from 'chai';
import { getCallRevertReason } from './common';
import {
    ZkSync,
    ZkSyncFactory,
    Proxy,
    UpgradeGatekeeper,
    Governance,
    Verifier,
    VerifierFactory,
    GovernanceFactory
} from '../typechain';

// The constants correspond to the mainnet configuration
const GOVERNANCE_PROXY_ADDRESS = '0x34460C0EB5074C29A9F6FE13b8e7E23A0D08aF01';
const VERIFIER_PROXY_ADDRESS = '0x5290E9582B4FB706EaDf87BB1c129e897e04d06D';
const ZK_SYNC_PROXY_ADDRESS = '0xaBEA9132b05A70803a4E85094fD0e1800777fBEF';
const UPGRADE_GATEKEEPER_ADDRESS = '0x38A43F4330f24fe920F943409709fc9A6084C939';
const UPGRADE_GATEKEEPER_MASTER_ADDRESS = '0xE24f4870Ab85DE8E356C5fC56138587206c70d99';

describe('Upgrade smart contracts fork test', function () {
    let upgradeGatekeeperOwner;
    let governanceProxy: Proxy;
    let verifierProxy: Proxy;
    let zkSyncProxy: Proxy;
    let upgradeGatekeeper: UpgradeGatekeeper;
    let newGovernanceTarget: Governance;
    let newVerifierTarget: Verifier;
    let newZkSyncTarget: ZkSync;

    before(async () => {
        // Access upgrade gatekeeper master account
        await hardhat.network.provider.send('hardhat_impersonateAccount', [UPGRADE_GATEKEEPER_MASTER_ADDRESS]);
        upgradeGatekeeperOwner = await hardhat.ethers.provider.getSigner(UPGRADE_GATEKEEPER_MASTER_ADDRESS);
        await hardhat.network.provider.send('hardhat_setBalance', [
            UPGRADE_GATEKEEPER_MASTER_ADDRESS,
            '0xfffffffffffffffff'
        ]);

        governanceProxy = (await hardhat.ethers.getContractAt('Proxy', GOVERNANCE_PROXY_ADDRESS)) as Proxy;
        verifierProxy = (await hardhat.ethers.getContractAt('Proxy', VERIFIER_PROXY_ADDRESS)) as Proxy;
        zkSyncProxy = (await hardhat.ethers.getContractAt('Proxy', ZK_SYNC_PROXY_ADDRESS)) as Proxy;
        upgradeGatekeeper = (await hardhat.ethers.getContractAt(
            'UpgradeGatekeeper',
            UPGRADE_GATEKEEPER_ADDRESS
        )) as UpgradeGatekeeper;

        const zkSyncFactory = await hardhat.ethers.getContractFactory('ZkSync');
        const zkSyncContract = await zkSyncFactory.deploy();
        newZkSyncTarget = ZkSyncFactory.connect(zkSyncContract.address, zkSyncContract.signer);

        const governanceFactory = await hardhat.ethers.getContractFactory('Governance');
        const governanceContract = await governanceFactory.deploy();
        newGovernanceTarget = GovernanceFactory.connect(governanceContract.address, governanceContract.signer);

        const verifierFactory = await hardhat.ethers.getContractFactory('Verifier');
        const verifierContract = await verifierFactory.deploy();
        newVerifierTarget = VerifierFactory.connect(verifierContract.address, verifierContract.signer);

        // If the upgrade is already running, then cancel it to start upgrading over.
        const currentUpgradeStatus = await upgradeGatekeeper.upgradeStatus();
        if (currentUpgradeStatus != 0) {
            await upgradeGatekeeper.connect(upgradeGatekeeperOwner).cancelUpgrade();
        }
    });

    it('check ownership invariants', async function () {
        expect(await upgradeGatekeeper.getMaster()).eq(UPGRADE_GATEKEEPER_MASTER_ADDRESS);
        expect(await governanceProxy.getMaster()).eq(upgradeGatekeeper.address);
        expect(await verifierProxy.getMaster()).eq(upgradeGatekeeper.address);
        expect(await zkSyncProxy.getMaster()).eq(upgradeGatekeeper.address);
    });

    it('should fail to start upgrade without permission', async () => {
        const { revertReason } = await getCallRevertReason(() =>
            upgradeGatekeeper.startUpgrade([
                newGovernanceTarget.address,
                newVerifierTarget.address,
                newZkSyncTarget.address
            ])
        );
        expect(revertReason).to.eq('oro11');
    });

    it('should start upgrade', async () => {
        const upgradeStatusBefore = await upgradeGatekeeper.upgradeStatus();
        await upgradeGatekeeper
            .connect(upgradeGatekeeperOwner)
            .startUpgrade([newGovernanceTarget.address, newVerifierTarget.address, newZkSyncTarget.address]);
        const upgradeStatusAfter = await upgradeGatekeeper.upgradeStatus();

        expect(upgradeStatusBefore).eq(0);
        expect(upgradeStatusAfter).eq(1);
    });

    it('should fail to start preparation until the end of the timelock without permission', async () => {
        const { revertReason } = await getCallRevertReason(() => upgradeGatekeeper.startPreparation());
        expect(revertReason).to.eq('oro11');
    });

    it('should fail to start preparation until the end of the timelock', async () => {
        const { revertReason } = await getCallRevertReason(() =>
            upgradeGatekeeper.connect(upgradeGatekeeperOwner).startPreparation()
        );
        expect(revertReason).to.eq('ups11');
    });

    it('should fail to start preparation after the end of the timelock without permission', async () => {
        await hardhat.network.provider.send('evm_increaseTime', [
            '0xfffffffffffffffff' // a lot of time for the timelock to pass accurately
        ]);

        const { revertReason } = await getCallRevertReason(() => upgradeGatekeeper.startPreparation());
        expect(revertReason).to.eq('oro11');
    });

    it('should start preparation after the end of the timelock', async () => {
        const upgradeStatusBefore = await upgradeGatekeeper.upgradeStatus();
        await upgradeGatekeeper.connect(upgradeGatekeeperOwner).startPreparation();
        const upgradeStatusAfter = await upgradeGatekeeper.upgradeStatus();

        expect(upgradeStatusBefore).eq(1);
        expect(upgradeStatusAfter).eq(2);
    });

    it('should fail to finish upgrade without permission', async () => {
        const { revertReason } = await getCallRevertReason(() => upgradeGatekeeper.finishUpgrade([[], [], []]));

        expect(revertReason).to.eq('oro11');
    });

    it('should finish upgrade', async () => {
        const governanceBefore = await governanceProxy.getTarget();
        const verifierBefore = await verifierProxy.getTarget();
        const zkSyncBefore = await zkSyncProxy.getTarget();

        const upgradeStatusBefore = await upgradeGatekeeper.upgradeStatus();
        await upgradeGatekeeper.connect(upgradeGatekeeperOwner).finishUpgrade([[], [], []]);
        const upgradeStatusAfter = await upgradeGatekeeper.upgradeStatus();

        const governanceAfter = await governanceProxy.getTarget();
        const verifierAfter = await verifierProxy.getTarget();
        const zkSyncAfter = await zkSyncProxy.getTarget();

        expect(upgradeStatusBefore).eq(2);
        expect(upgradeStatusAfter).eq(0);

        expect(governanceBefore).not.eq(newGovernanceTarget.address);
        expect(verifierBefore).not.eq(newVerifierTarget.address);
        expect(zkSyncBefore).not.eq(newZkSyncTarget.address);

        expect(governanceAfter).eq(newGovernanceTarget.address);
        expect(verifierAfter).eq(newVerifierTarget.address);
        expect(zkSyncAfter).eq(newZkSyncTarget.address);
    });
});
