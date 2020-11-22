import { ethers } from 'hardhat';
import { Signer } from 'ethers';
import { Governance, ZkSync } from '../typechain';
import { BytesLike } from '@ethersproject/bytes';

describe('Token', function () {
    let accounts: Signer[];
    let zkSync: ZkSync;
    let gov: Governance;

    before(async function () {
        accounts = await ethers.getSigners();
        const zksyncFactory = await ethers.getContractFactory('ZkSync');
        zkSync = ((await zksyncFactory.deploy()) as unknown) as ZkSync;

        const govFactory = await ethers.getContractFactory('Governance');
        gov = ((await govFactory.deploy()) as unknown) as Governance;
        const governor = await accounts[0].getAddress();
        await gov.initialize(ethers.utils.defaultAbiCoder.encode(['address'], [governor]));
        await gov.setValidator(governor, true);

        const zeroBlock = {
            processableOnchainOperationsHash: ethers.constants.HashZero,
            stateHash: ethers.constants.HashZero,
            commitment: ethers.constants.HashZero
        };

        const zeroBlockStoredHash = ethers.utils.keccak256(
            ethers.utils.solidityPack(
                ['bytes32', 'bytes32', 'bytes32'],
                [zeroBlock.processableOnchainOperationsHash, zeroBlock.stateHash, zeroBlock.commitment]
            )
        );
        await zkSync.initialize(
            ethers.utils.defaultAbiCoder.encode(
                ['address', 'address', 'bytes32'],
                [gov.address, ethers.constants.AddressZero, zeroBlockStoredHash]
            )
        );
    });

    it('commit', async function () {
        const zeroBlockStored = {
            blockNumber: 0,
            processableOnchainOperationsHash: ethers.constants.HashZero,
            stateHash: ethers.constants.HashZero,
            commitment: ethers.constants.HashZero
        };

        const blockOne = {
            blockNumber: 1,
            feeAccount: 0,
            newStateRoot: ethers.constants.HashZero,
            publicData: '0x05aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            onchainOperations: []
        };
        await zkSync.commitBlocks(zeroBlockStored, [blockOne]);

        const oneBlockStored = {
            blockNumber: 1,
            processableOnchainOperationsHash: ethers.utils.keccak256('0x'),
            stateHash: ethers.constants.HashZero,
            commitment: ethers.constants.HashZero
        };

        const blocks = [];
        for (let i = 0; i < 2; ++i) {
            blocks.push({
                blockNumber: 2 + i,
                feeAccount: 0,
                newStateRoot: ethers.constants.HashZero,
                publicData: '0x05aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                onchainOperations: []
            });
        }

        let tx = await zkSync.commitBlocks(oneBlockStored, blocks);
        let receipt = await tx.wait();
        console.log('commit', receipt.gasUsed.toString());

        for (let i = 0; i < 2; ++i) {
            const blockData = blocks[i];
            blocks[i] = {
                storedBlock: {
                    blockNumber: blockData.blockNumber,
                    processableOnchainOperationsHash: ethers.utils.keccak256('0x'),
                    stateHash: ethers.constants.HashZero,
                    commitment: ethers.constants.HashZero
                },
                onchainOpsPubdata: [],
                commitmentsInSlot: [ethers.utils.keccak256('0x')],
                commitmentIndex: 0
            };
        }

        tx = await zkSync.executeBlocks(blocks);
        receipt = await tx.wait();
        console.log('execute', receipt.gasUsed.toString());
    });
});
