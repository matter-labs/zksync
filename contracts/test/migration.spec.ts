import * as hardhat from 'hardhat';
import { expect } from 'chai';
import { constants } from 'ethers';
import { parseEther, solidityKeccak256 } from 'ethers/lib/utils';
import { Deployer, readContractCode, readProductionContracts } from '../src.ts/deploy';
import { ZkSyncWithdrawalUnitTestFactory } from '../typechain';

describe('zkSync token migration unit tests', function () {
    this.timeout(50000);

    let networkGovernor: any;
    let zksyncContract: any;
    let tokenContract: any;
    let l1ClaimDistributor: any;

    const claimRoot = solidityKeccak256(['string'], ['claim-root']);

    async function deployDistributor(merkleRoot: string) {
        const factory = await hardhat.ethers.getContractFactory('MockMigrationClaimDistributorL1');
        return factory.deploy(merkleRoot);
    }

    async function initSecurityCouncilMultisig(deployer: Deployer, multisig: string) {
        const upgradeGatekeeper = deployer.upgradeGatekeeperContract(networkGovernor);
        const zkSyncTarget = deployer.addresses.ZkSyncTarget;

        await upgradeGatekeeper.startUpgrade([constants.AddressZero, constants.AddressZero, zkSyncTarget]);
        await upgradeGatekeeper.startPreparation();
        // The Security-Council multisig is now baked into the ZkSync bytecode at
        // compile time, so finishUpgrade no longer carries it. In tests we override
        // the baked-in value via the ZkSyncWithdrawalUnitTest helper further down.
        await upgradeGatekeeper.finishUpgrade(['0x', '0x', '0x']);
        await zksyncContract.setSecurityCouncilMultisig(multisig);
    }

    beforeEach(async () => {
        [networkGovernor] = await hardhat.ethers.getSigners();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncWithdrawalUnitTest');
        const deployer = new Deployer({ deployWallet: networkGovernor as any, contracts });
        await deployer.deployAll({ gasLimit: 8000000 });

        zksyncContract = ZkSyncWithdrawalUnitTestFactory.connect(deployer.addresses.ZkSync, networkGovernor);

        await initSecurityCouncilMultisig(deployer, await networkGovernor.getAddress());
        // The `upgrade` function resets `additionalZkSync` to the compile-time env-baked address,
        // which does not match the Create2-deployed one in tests. Restore it so delegated calls work.
        await zksyncContract.setAdditionalZkSync(deployer.addresses.AdditionalZkSync);

        const tokenContractFactory = await hardhat.ethers.getContractFactory('TestnetERC20Token');
        tokenContract = await tokenContractFactory.deploy('Matter Labs Trial Token', 'MLTT', 18);
        await tokenContract.mint(await networkGovernor.getAddress(), parseEther('1000000'));

        l1ClaimDistributor = await deployDistributor(claimRoot);

        const govContract = deployer.governanceContract(networkGovernor as any);
        await govContract.addToken(tokenContract.address);
    });

    it('sets distributor, pulls claim root, and activates exodus mode', async () => {
        await expect(zksyncContract.setClaimRoot(l1ClaimDistributor.address))
            .to.emit(zksyncContract, 'L1ClaimDistributorSet')
            .withArgs(l1ClaimDistributor.address)
            .and.to.emit(zksyncContract, 'ClaimRootSet')
            .withArgs(claimRoot)
            .and.to.emit(zksyncContract, 'ExodusMode');

        expect(await zksyncContract.exodusMode()).to.eq(true);
    });

    it('does not re-emit ExodusMode when the distributor is updated', async () => {
        await zksyncContract.setClaimRoot(l1ClaimDistributor.address);
        const anotherDistributor = await deployDistributor(solidityKeccak256(['string'], ['another-root']));
        await expect(zksyncContract.setClaimRoot(anotherDistributor.address)).to.not.emit(zksyncContract, 'ExodusMode');
    });

    it('rejects a zero distributor address', async () => {
        await expect(zksyncContract.setClaimRoot(constants.AddressZero)).to.be.revertedWith('tm1');
    });

    it('rejects a distributor whose MERKLE_ROOT is zero', async () => {
        const emptyDistributor = await deployDistributor(constants.HashZero);
        await expect(zksyncContract.setClaimRoot(emptyDistributor.address)).to.be.revertedWith('tm2');
    });

    it('migrates native token balance to the L1 claim distributor', async () => {
        const ethTotal = parseEther('3.75');
        await zksyncContract.receiveETH({ value: ethTotal });
        await zksyncContract.setClaimRoot(l1ClaimDistributor.address);

        const distributorBalanceBefore = await hardhat.ethers.provider.getBalance(l1ClaimDistributor.address);

        await expect(zksyncContract.migrateToken(constants.AddressZero))
            .to.emit(zksyncContract, 'TokenMigrationExecuted')
            .withArgs(constants.AddressZero);

        expect(await hardhat.ethers.provider.getBalance(l1ClaimDistributor.address)).to.eq(
            distributorBalanceBefore.add(ethTotal)
        );
        expect(await hardhat.ethers.provider.getBalance(zksyncContract.address)).to.eq(0);
        expect(await zksyncContract.isTokenMigrated(constants.AddressZero)).to.eq(true);
    });

    it('migrates ERC20 token balance to the L1 claim distributor', async () => {
        const erc20Total = parseEther('10');
        await tokenContract.transfer(zksyncContract.address, erc20Total);
        await zksyncContract.setClaimRoot(l1ClaimDistributor.address);

        const distributorBalanceBefore = await tokenContract.balanceOf(l1ClaimDistributor.address);

        await expect(zksyncContract.migrateToken(tokenContract.address))
            .to.emit(zksyncContract, 'TokenMigrationExecuted')
            .withArgs(tokenContract.address);

        expect(await tokenContract.balanceOf(l1ClaimDistributor.address)).to.eq(
            distributorBalanceBefore.add(erc20Total)
        );
        expect(await tokenContract.balanceOf(zksyncContract.address)).to.eq(0);
        expect(await zksyncContract.isTokenMigrated(tokenContract.address)).to.eq(true);
    });

    it('rejects a migration when the L1 claim distributor is not configured', async () => {
        await expect(zksyncContract.migrateToken(constants.AddressZero)).to.be.revertedWith('tm1');
    });

    it('rejects duplicate migrations for the same token', async () => {
        const erc20Total = parseEther('3');
        await tokenContract.transfer(zksyncContract.address, erc20Total);
        await zksyncContract.setClaimRoot(l1ClaimDistributor.address);

        await zksyncContract.migrateToken(tokenContract.address);
        await expect(zksyncContract.migrateToken(tokenContract.address)).to.be.revertedWith('tm3');
    });

    it('rejects migrations when the contract holds no balance of the token', async () => {
        await zksyncContract.setClaimRoot(l1ClaimDistributor.address);
        await expect(zksyncContract.migrateToken(tokenContract.address)).to.be.revertedWith('tm5');
        await expect(zksyncContract.migrateToken(constants.AddressZero)).to.be.revertedWith('tm5');
    });
});
