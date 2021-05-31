import { RegenesisMultisigFactory, UpgradeGatekeeperFactory, ZkSyncRegenesisTestFactory } from '../../typechain';
import { ethers, utils } from 'ethers';
import { storedBlockInfoParam } from '../../scripts/utils';
const { expect } = require('chai');
const hardhat = require('hardhat');
import { Deployer, readContractCode, readProductionContracts } from '../../src.ts/deploy';

describe('Regenesis test', function () {
    this.timeout(50000);

    // Not sure about different hardhat versions' wallets,
    // so it is better to always deploy the multisig with the same address to
    // preserve the contract's address
    const walletPrivateKey = '0x6878e5113d9fae7eec373bd9f7975e692c1c4ace22b536c63aa2c818ef92ef00';
    const wallet = new ethers.Wallet(walletPrivateKey).connect(hardhat.ethers.provider);

    it('Test that regenesis upgrade works', async () => {
        // Fund the deployer wallet
        const hardhatWallets = await hardhat.ethers.getSigners();
        const hardhatWallet: ethers.Wallet = hardhatWallets[0];

        const supplyMultisigCreatorTx = await hardhatWallet.sendTransaction({
            to: wallet.address,
            value: utils.parseEther('10')
        });
        await supplyMultisigCreatorTx.wait();

        // Deploying the contracts
        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncRegenesisTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployRegenesisMultisig({ gasLimit: 6500000 });
        await deployer.deployAll({ gasLimit: 6500000 });

        const regenesisMultisigContract = RegenesisMultisigFactory.connect(
            deployer.addresses.RegenesisMultisig,
            wallet
        );
        const zksyncContract = ZkSyncRegenesisTestFactory.connect(deployer.addresses.ZkSync, wallet);
        const governanceAdress = deployer.addresses.GovernanceTarget;
        const verifierAddrss = deployer.addresses.VerifierTarget;
        const zkSyncAddress = deployer.addresses.ZkSyncTarget;
        const upgradeGatekeeperContract = UpgradeGatekeeperFactory.connect(
            deployer.addresses.UpgradeGatekeeper,
            wallet
        );

        // Starting upgrade
        await expect(upgradeGatekeeperContract.startUpgrade([governanceAdress, verifierAddrss, zkSyncAddress])).to.emit(
            upgradeGatekeeperContract,
            'NoticePeriodStart'
        );
        await expect(upgradeGatekeeperContract.startPreparation()).to.emit(
            upgradeGatekeeperContract,
            'PreparationStart'
        );

        const oldRootHash = process.env.CONTRACTS_GENESIS_ROOT;
        expect(oldRootHash).to.eq(
            '0x2d5ab622df708ab44944bb02377be85b6f27812e9ae520734873b7a193898ba4',
            'The test requires a specific GENESIS_ROOT'
        );
        const newRootHash = '0x2a9b50e17ece607c8c88b1833426fd9e60332685b94a1534fcf26948e373604c';

        const submitSignaturesTx = await regenesisMultisigContract.submitHash(oldRootHash, newRootHash);
        await submitSignaturesTx.wait();

        // After the new root hash has been submitted to the multisig,
        // we need to finish regenesis
        const genesisBlock = {
            blockNumber: 0,
            priorityOperations: 0,
            pendingOnchainOperationsHash: '0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470',
            timestamp: 0,
            stateHash: oldRootHash,
            commitment: '0x0000000000000000000000000000000000000000000000000000000000000000'
        };

        const StoredBlockInfo = storedBlockInfoParam();

        // We need some address, but it is not needed for upgrade itself, so we don't care
        const additionalZkSyncAddress = process.env.MISC_NEW_ADDITIONAL_ZKSYNC_ADDRESS;

        const encodedUpgradeData = ethers.utils.defaultAbiCoder.encode([StoredBlockInfo], [genesisBlock]);

        const tx = await upgradeGatekeeperContract.finishUpgrade([[], [], encodedUpgradeData]);
        await tx.wait();

        const newBlock = {
            ...genesisBlock,
            stateHash: '0x2a9b50e17ece607c8c88b1833426fd9e60332685b94a1534fcf26948e373604c'
        };

        const newBlockData = ethers.utils.defaultAbiCoder.encode([StoredBlockInfo], [newBlock]);

        const expectedNewBlockHash = ethers.utils.keccak256(newBlockData);
        const newBlockHash = await zksyncContract.getStoredBlockHash();
        const newAdditionalZkSyncAddress = await zksyncContract.getAdditionalZkSync();
        expect(expectedNewBlockHash).to.eq(newBlockHash, 'The new block has been applied wrongly');
        expect(additionalZkSyncAddress.toLowerCase()).to.eq(
            newAdditionalZkSyncAddress.toLowerCase(),
            'The additional zkSync address has been changed wrongly'
        );
    });

    it('Test data submission', async () => {
        const [hardhatWallet]: ethers.Wallet[] = await hardhat.ethers.getSigners();
        const fundingWalletTx = await hardhatWallet.sendTransaction({
            to: wallet.address,
            value: utils.parseEther('3.0')
        });
        await fundingWalletTx.wait();

        const contracts = readProductionContracts();
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployRegenesisMultisig({ gasLimit: 6500000 });
        await deployer.deployAll({ gasLimit: 6500000 });

        const regenesisMultisigContract = RegenesisMultisigFactory.connect(
            deployer.addresses.RegenesisMultisig,
            wallet
        );

        const tx = await wallet.sendTransaction({
            to: regenesisMultisigContract.address,
            // The calldata was retrieved from the regen-root-hash tool
            data: '0x905717402f59c906954c0445843de5e33ceb41d60b5ed5d3d78f0575bc345bd3514ea0910c0c243023dce4bb411344d572dcc24bd77d393ef5a02ef4f5ffd12649634d5e'
        });
        await tx.wait();

        expect(await regenesisMultisigContract.oldRootHash()).to.eq(
            '0x2f59c906954c0445843de5e33ceb41d60b5ed5d3d78f0575bc345bd3514ea091',
            'The old root hash was not set correctly'
        );
        expect(await regenesisMultisigContract.newRootHash()).to.eq(
            '0x0c0c243023dce4bb411344d572dcc24bd77d393ef5a02ef4f5ffd12649634d5e',
            'The new root hash was not set correctly'
        );
    });
});
