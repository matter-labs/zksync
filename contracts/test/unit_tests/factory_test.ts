import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Signer } from 'ethers';
const { getCallRevertReason } = require('./common');
import { ZkSyncNFTFactory } from '../../typechain/ZkSyncNFTFactory';
import { ZkSyncNFTFactoryFactory } from '../../typechain/ZkSyncNFTFactoryFactory';

import * as hardhat from 'hardhat';

use(solidity);

describe('NFTFactory unit tests', function () {
    this.timeout(50000);

    let contract;
    let nftFactory: ZkSyncNFTFactory;
    let wallet1: Signer;
    let wallet2: Signer;

    before(async () => {
        [wallet1, wallet2] = await hardhat.ethers.getSigners();

        const nftFactoryFactory = await hardhat.ethers.getContractFactory('ZkSyncNFTFactory');
        contract = await nftFactoryFactory.deploy('test', 'TS', wallet1.getAddress());
        // Connecting the wallet to a potential receiver, who can withdraw the funds
        // on the master's behalf
    });

    it('Success', async () => {
        // The test checks the ability to mint NFT from allowed contract
        const address = await wallet2.getAddress();
        const contentHash = '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996';
        nftFactory = ZkSyncNFTFactoryFactory.connect(contract.address, wallet1);
        await nftFactory.mintNFTFromZkSync(address, 1, address, 10, contentHash, 10);
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
                1,
                address,
                10,
                '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996',
                10
            )
        );
        expect(revertReason).equal('z');
    });
});
