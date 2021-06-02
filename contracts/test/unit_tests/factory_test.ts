import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { BigNumber, BigNumberish, ethers, Signer } from 'ethers';
const { getCallRevertReason } = require('./common');
import { ZkSyncNFTFactory } from '../../typechain/ZkSyncNFTFactory';
import { ZkSyncNFTFactoryFactory } from '../../typechain/ZkSyncNFTFactoryFactory';
import { ZkSyncNFTFactoryUnitTest, ZkSyncNFTFactoryUnitTestFactory } from '../../typechain';

import * as hardhat from 'hardhat';

use(solidity);

describe('NFTFactory unit tests', function () {
    this.timeout(50000);

    let contract;
    let nftFactory: ZkSyncNFTFactory;

    let unitTestContract: ZkSyncNFTFactoryUnitTest;

    let wallet1: Signer;
    let wallet2: Signer;

    before(async () => {
        [wallet1, wallet2] = await hardhat.ethers.getSigners();

        const nftFactoryFactory = await hardhat.ethers.getContractFactory('ZkSyncNFTFactory');
        contract = await nftFactoryFactory.deploy('test', 'TS', wallet1.getAddress());

        const unitTestContractFactory = new ZkSyncNFTFactoryUnitTestFactory(wallet1);
        unitTestContract = await unitTestContractFactory.deploy('NFT', 'DEFAULT', ethers.constants.AddressZero);
        // Connecting the wallet to a potential receiver, who can withdraw the funds
        // on the master's behalf
    });

    it('Success', async () => {
        // The test checks the ability to mint NFT from allowed contract
        const address = await wallet2.getAddress();
        const contentHash = '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996';
        nftFactory = ZkSyncNFTFactoryFactory.connect(contract.address, wallet1);
        await nftFactory.mintNFTFromZkSync(address, address, 1, 10, contentHash, 10);
        const owner = await nftFactory.ownerOf(10);
        expect(owner).to.equal(await wallet2.getAddress());

        // Checking saved metadata
        expect(await nftFactory.getContentHash(10)).to.eq(contentHash, 'Content hash is not correct');
        expect(await nftFactory.getCreatorAddress(10)).to.eq(address, 'Address is incorrect');
        expect(await nftFactory.getCreatorAccountId(10)).to.eq(1, 'Account Id is incorrect');
        expect(await nftFactory.getSerialId(10)).to.eq(10, 'Serial Id is incorrect');
    });
    it('Error', async () => {
        // The test checks the ability to mint NFT from allowed contract
        nftFactory = ZkSyncNFTFactoryFactory.connect(contract.address, wallet2);
        const address = await wallet2.getAddress();
        const { revertReason } = await getCallRevertReason(() =>
            nftFactory.mintNFTFromZkSync(
                address,
                address,
                1,
                10,
                '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996',
                10
            )
        );
        expect(revertReason).equal('z');
    });

    it('Bit operations', async () => {
        const oneTest = async (
            number: BigNumberish,
            firstBit: BigNumberish,
            lastBit: BigNumberish,
            expectedOutcome: BigNumberish
        ) => {
            const bits = await unitTestContract.getBitsPublic(number, firstBit, lastBit);

            expect(bits.eq(expectedOutcome)).to.eq(true, 'Getting bits does not work');
        };

        // 7 = 1110000000...
        // Getting bits from the first one to the third one (the range is exclusive)
        // means getting bits
        // 1[110]00000...
        // 110 = 1 + 2 = 3;
        await oneTest(7, 1, 4, 3);

        // 128 = 2^7
        // Getting the seventh bit should return 1
        await oneTest(128, 7, 8, 1);

        const two_pow_190 = BigNumber.from(2).pow(190);
        const two_pow_193 = BigNumber.from(2).pow(193);
        // The range is exclusive
        await oneTest(two_pow_190.add(two_pow_193), 190, 193, 1);

        const two_pow_191 = BigNumber.from(2).pow(191);
        const two_pow_200 = BigNumber.from(2).pow(200);
        // Taking all the bits
        await oneTest(two_pow_191.add(two_pow_200), 0, 256, two_pow_191.add(two_pow_200));
    });
});
