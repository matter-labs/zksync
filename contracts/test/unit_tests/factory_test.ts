import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Signer } from 'ethers';
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
        nftFactory = ZkSyncNFTFactoryFactory.connect(contract.address, wallet1);
        await nftFactory.mintNFT(
            await wallet2.getAddress(),
            await wallet2.getAddress(),
            '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996',
            10
        );
        const owner = await nftFactory.ownerOf(10);
        expect(owner).to.equal(await wallet2.getAddress());
    });
    it('Error', async () => {
        // The test checks the ability to mint NFT from allowed contract
        nftFactory = ZkSyncNFTFactoryFactory.connect(contract.address, wallet2);

        expect(
            await nftFactory.mintNFT(
                await wallet2.getAddress(),
                await wallet2.getAddress(),
                '0xbd7289936758c562235a3a42ba2c4a56cbb23a263bb8f8d27aead80d74d9d996',
                10
            )
        ).to.throw('Something');
    });
});
