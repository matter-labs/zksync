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

        // Submitting signatures to the multisig
        expect(await regenesisMultisigContract.numberOfPartners()).to.eq(4, 'The test is aimed at 4 partners');
        expect(await regenesisMultisigContract.requiredNumberOfSignatures()).to.eq(
            3,
            'The test is aimed at 3 required signatures'
        );

        const oldRootHash = process.env.CONTRACTS_GENESIS_ROOT;
        expect(oldRootHash).to.eq(
            '0x2d5ab622df708ab44944bb02377be85b6f27812e9ae520734873b7a193898ba4',
            'The test requires a specific GENESIS_ROOT'
        );
        const newRootHash = '0x2a9b50e17ece607c8c88b1833426fd9e60332685b94a1534fcf26948e373604c';
        const signatures = [
            // Correct signature for 0x374Ac2A10cBCaE93d2aBBe468f0EDEF6768e65eE
            '0xeae499cb52c214e998ec9311e883f9362d8f0e2448e1c2275ebacd2ad92679751e79d8e9f7ed6ff513afc55ac5acf09bd4f1b6b893e0fb849c89cf3d25d091341c',
            // Correct signature for 0xB991c776AedacfA5a7e8CF3e7aD6CB6C1AcB9227
            '0x23e85b70fdbcb1eaeacf83a3a62c5bbfb604bd34f8ef9798f05fe915ad5d3cc6661c7f941dc7656c63ee28bbec760b32cbb893cc04560ff433eacc92aecec60a1c',
            // Incorrect signature
            '0x4843ef9f8e9bb01c883b4df5b99a5287d4602e6340f9d4207900af5a333b5d90186cb5267f8789e6364d2fa737778e13c129b295fdc5f220d4bfa03e948f262e1b',
            // Correct signature for 0x093Cf8450c5eE506aB865F68F5f8EB8C4C2073C2
            '0xb0b6e1efbca8abd97a4cb96c19ef59f6640c10b9369e2d89111a8f0622a0b0c249a0bef1d4479a98c85832926ef04072ff2e9f51fde53967ce235971995629001c'
        ];

        const submitSignaturesTx = await regenesisMultisigContract.submitSignatures(
            oldRootHash,
            newRootHash,
            signatures
        );
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
        const additionalZkSyncAddress = '0xc0f97CC918C9d6fA4E9fc6be61a6a06589D199b2'; 

        const encodedStoredBlockInfo = ethers.utils.defaultAbiCoder.encode(
            [StoredBlockInfo, 'address'], [genesisBlock, additionalZkSyncAddress]);

        const tx = await upgradeGatekeeperContract.finishUpgrade([[], [], encodedStoredBlockInfo]);
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
        expect(additionalZkSyncAddress).to.eq(newAdditionalZkSyncAddress, 'The additional zkSync address has been changed wrongly');
    });
});
