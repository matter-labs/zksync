import {
    Governance,
    GovernanceFactory,
    Proxy,
    ProxyFactory,
    RegenesisMultisigFactory,
    UpgradeGatekeeper,
    UpgradeGatekeeperFactory,
    Verifier,
    VerifierFactory,
    ZkSync,
    ZkSyncFactory,
    ZkSyncRegenesisTest,
    ZkSyncRegenesisTestFactory
} from '../../typechain';
import { BigNumber, constants, ethers, utils } from 'ethers';
const { expect } = require('chai');
const { getCallRevertReason } = require('./common');
const hardhat = require('hardhat');
const { performance } = require('perf_hooks');
import { Deployer, readContractCode, readProductionContracts } from '../../src.ts/deploy';
import { ParamType } from '@ethersproject/abi';

const StoredBlockInfoAbi = {
    components: [
        {
            internalType: 'uint32',
            name: 'blockNumber',
            type: 'uint32'
        },
        {
            internalType: 'uint64',
            name: 'priorityOperations',
            type: 'uint64'
        },
        {
            internalType: 'bytes32',
            name: 'pendingOnchainOperationsHash',
            type: 'bytes32'
        },
        {
            internalType: 'uint256',
            name: 'timestamp',
            type: 'uint256'
        },
        {
            internalType: 'bytes32',
            name: 'stateHash',
            type: 'bytes32'
        },
        {
            internalType: 'bytes32',
            name: 'commitment',
            type: 'bytes32'
        }
    ],
    internalType: 'struct Storage.StoredBlockInfo',
    name: '_lastCommittedBlockData',
    type: 'tuple'
};

describe.only('Regenesis test', function () {
    this.timeout(50000);

    // Not sure about different hardhat versions' wallets,
    // so it is better to always deploy the multisig with the same address to
    // preserve the contract's address 
    const walletPrivateKey = '0x6878e5113d9fae7eec373bd9f7975e692c1c4ace22b536c63aa2c818ef92ef00';
    const wallet = (new ethers.Wallet(walletPrivateKey)).connect(hardhat.ethers.provider);

    it('Test that regenesis upgrade works', async () => {
        const hardhatWallets = await hardhat.ethers.getSigners();
        // Get some wallet different from than the default one.
        const hardhatWallet: ethers.Wallet = hardhatWallets[0];

        const supplyMultisigCreatorTx = await hardhatWallet.sendTransaction({
            to: wallet.address,
            value: utils.parseEther('10')
        });
        await supplyMultisigCreatorTx.wait();

        const contracts = readProductionContracts();
        contracts.zkSync = readContractCode('dev-contracts/ZkSyncRegenesisTest');
        const deployer = new Deployer({ deployWallet: wallet, contracts });
        await deployer.deployRegenesisMultisig({ gasLimit: 6500000 });
        await deployer.deployAll({ gasLimit: 6500000 });

        const regenesisMultisigContract = RegenesisMultisigFactory.connect(deployer.addresses.RegenesisMultisig, wallet);
        const zksyncContract = ZkSyncRegenesisTestFactory.connect(deployer.addresses.ZkSync, wallet);

        const governanceAdress = deployer.addresses.GovernanceTarget;
        const verifierAddrss = deployer.addresses.VerifierTarget;
        const zkSyncAddress = deployer.addresses.ZkSyncTarget;
        const upgradeGatekeeperContract = UpgradeGatekeeperFactory.connect(
            deployer.addresses.UpgradeGatekeeper,
            wallet
        );

        // Starting upgrade...
        await expect(upgradeGatekeeperContract.startUpgrade([governanceAdress, verifierAddrss, zkSyncAddress])).to.emit(
            upgradeGatekeeperContract,
            'NoticePeriodStart'
        );

        await expect(upgradeGatekeeperContract.startPreparation()).to.emit(
            upgradeGatekeeperContract,
            'PreparationStart'
        );

        // Submitting signatures to the multisig
        expect(await regenesisMultisigContract.NUMBER_OF_PARTNERS()).to.eq(4, 'The test is aimed at 4 partners');
        expect(await regenesisMultisigContract.requiredNumberOfSignatures()).to.eq(3, 'The test is aimed at 3 required signatures');

        const oldRootHash = process.env.CONTRACTS_GENESIS_ROOT;
        const newRootHash = '0x2a9b50e17ece607c8c88b1833426fd9e60332685b94a1534fcf26948e373604c';
        const signatures = [
            // Correct signature for 0x374Ac2A10cBCaE93d2aBBe468f0EDEF6768e65eE
            '0x79cd9bc179b7baa157c8994e829fabeac72e203df7be9e4180a6d56a95b79c9d528e4c2bdba8097f0dd0b0852299dd42e362f4de45726f89b897d52250aa13271b',
            // Correct signature for 0xB991c776AedacfA5a7e8CF3e7aD6CB6C1AcB9227
            '0x23e85b70fdbcb1eaeacf83a3a62c5bbfb604bd34f8ef9798f05fe915ad5d3cc6661c7f941dc7656c63ee28bbec760b32cbb893cc04560ff433eacc92aecec60a1c',
            // Incorrect signature
            '0x19cd9bc179b7baa157c8994e829fabeac72e203df7be9e4180b6d56a95b79c9d528e4c2bdba8097f0dd2b0852299dd42e362f4de45726f89b897d52250aa13271b',
            // Correct signature for 0x093Cf8450c5eE506aB865F68F5f8EB8C4C2073C2
            '0x2facb8611a6d69afe4b37a75cd5c8210b69620cdeb34f716ad38ba13fd317c3d59dabfadab03fadae4b741fd63f313e500e928f83d52cb9cfb3557ff0c2ab7991b'
        ];

        const submitSignaturesTx = await regenesisMultisigContract.submitSignatures(
            oldRootHash,
            newRootHash,
            signatures
        );
        await submitSignaturesTx.wait();

        const genesisBlock = {
            blockNumber: 0,
            priorityOperations: 0,
            pendingOnchainOperationsHash: '0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470',
            timestamp: 0,
            stateHash: '0x217e7707684bd964d4482cf1a10ea7c0eb6b5d6d1e41accbd75c571284a93cd0',
            commitment: '0x0000000000000000000000000000000000000000000000000000000000000000'
        };

        const StoredBlockInfo = ParamType.fromObject(StoredBlockInfoAbi);

        const encodedStoredBlockInfo = ethers.utils.defaultAbiCoder.encode(
            [StoredBlockInfo],
            [genesisBlock]
        );

        const tx = await upgradeGatekeeperContract.finishUpgrade([[], [], encodedStoredBlockInfo]);
        const receipt = await tx.wait();

        const timestamp = (await wallet.provider.getBlock(receipt.blockNumber)).timestamp;

        const newBlock = {
            blockNumber: 1,
            priorityOperations: 0,
            pendingOnchainOperationsHash: '0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470',
            timestamp: timestamp,
            stateHash: '0x2a9b50e17ece607c8c88b1833426fd9e60332685b94a1534fcf26948e373604c',
            commitment: '0x0000000000000000000000000000000000000000000000000000000000000000'
        };

        const newBlockData = ethers.utils.defaultAbiCoder.encode([StoredBlockInfo], [newBlock]);

        const expectedNewBlockHash = ethers.utils.keccak256(newBlockData);
        const newBlockHash = await zksyncContract.getStoredBlockHash();
        expect(expectedNewBlockHash).to.eq(newBlockHash, 'The new block has been applied wrongly');
    });
});
