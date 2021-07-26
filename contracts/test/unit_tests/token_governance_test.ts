import { ethers } from 'ethers';

const hardhat = require('hardhat');
import {
    GovernanceFactory,
    TokenGovernanceFactory,
    TokenGovernance,
    Governance,
    TestnetERC20TokenFactory,
    TestnetERC20Token
} from '../../typechain';

import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';

use(solidity);

describe('ZK token governance unit tests', function () {
    this.timeout(50000);

    const REQUIRE_ZKSYNC_GOVERNOR = '1g';

    const LISTING_FEE = 250; // payment for token addition
    const MAX_TOKEN = 2; // Can add only 2 tokens using token governance
    const ERC20_ADDRESS_1 = '0x0000000000000000000000000000000000000001';
    const ERC20_ADDRESS_2 = '0x0000000000000000000000000000000000000002';
    const ERC20_ADDRESS_3 = '0x0000000000000000000000000000000000000003';
    const ERC20_ADDRESS_4 = '0x0000000000000000000000000000000000000004';

    let zkSyncGovernor;
    let zkSyncGovernance: Governance;
    let tokenGovernance: TokenGovernance;
    let tokenLister;
    let userWallet;
    let treasury;
    let paymentToken: TestnetERC20Token;
    before(async () => {
        [zkSyncGovernor, tokenLister, userWallet, treasury] = await hardhat.ethers.getSigners();

        const tokFactory = new TestnetERC20TokenFactory(zkSyncGovernor);
        paymentToken = await tokFactory.deploy('DAI', 'DAI', 18);

        const govFactory = new GovernanceFactory(zkSyncGovernor);
        zkSyncGovernance = await govFactory.deploy();
        await zkSyncGovernance.initialize(ethers.utils.defaultAbiCoder.encode(['address'], [zkSyncGovernor.address]));

        const tokGovFactory = new TokenGovernanceFactory(zkSyncGovernor);
        tokenGovernance = await tokGovFactory.deploy(
            zkSyncGovernance.address,
            paymentToken.address,
            LISTING_FEE,
            MAX_TOKEN,
            treasury.address
        );
    });

    it('Governor can change token governance', async () => {
        await expect(
            zkSyncGovernance.connect(userWallet).changeTokenGovernance(tokenGovernance.address)
        ).to.be.revertedWith(REQUIRE_ZKSYNC_GOVERNOR);

        const previousTokGovAddress = await zkSyncGovernance.tokenGovernance();
        expect(previousTokGovAddress).to.eq(ethers.constants.AddressZero);

        await expect(zkSyncGovernance.connect(zkSyncGovernor).changeTokenGovernance(tokenGovernance.address))
            .to.emit(zkSyncGovernance, 'NewTokenGovernance')
            .withArgs(tokenGovernance.address);

        const newTokGovAddress = await zkSyncGovernance.tokenGovernance();
        expect(newTokGovAddress).to.eq(tokenGovernance.address);
    });

    it('Governor can list tokens for free', async () => {
        const newTokenId = (await zkSyncGovernance.totalTokens()) + 1;
        await expect(tokenGovernance.connect(zkSyncGovernor).addToken(ERC20_ADDRESS_1))
            .to.emit(zkSyncGovernance, 'NewToken')
            .withArgs(ERC20_ADDRESS_1, newTokenId);
    });

    it('User should pay fee for listing', async () => {
        await expect(tokenGovernance.connect(userWallet).addToken(ERC20_ADDRESS_2)).to.be.revertedWith(
            'fee transfer failed'
        );
    });

    it('User can pay for listing and add token', async () => {
        await paymentToken.mint(userWallet.address, LISTING_FEE);
        await paymentToken.connect(userWallet).approve(tokenGovernance.address, LISTING_FEE);

        const newTokenId = (await zkSyncGovernance.totalTokens()) + 1;
        await expect(() =>
            expect(tokenGovernance.connect(userWallet).addToken(ERC20_ADDRESS_2))
                .to.emit(zkSyncGovernance, 'NewToken')
                .withArgs(ERC20_ADDRESS_2, newTokenId)
        ).to.changeTokenBalances(paymentToken, [userWallet, treasury], [-LISTING_FEE, LISTING_FEE]);
    });

    it('Cant add more than listingCap tokens', async () => {
        await expect(tokenGovernance.connect(zkSyncGovernor).addToken(ERC20_ADDRESS_3)).to.be.revertedWith(
            "can't add more tokens"
        );
    });

    it('Set listing token', async () => {
        await expect(
            tokenGovernance.connect(userWallet).setListingFeeToken(ethers.constants.AddressZero, 1)
        ).to.be.revertedWith(REQUIRE_ZKSYNC_GOVERNOR);

        await tokenGovernance.connect(zkSyncGovernor).setListingFeeToken(ethers.constants.AddressZero, 1);

        expect(await tokenGovernance.listingFee()).to.eq(1);
        expect(await tokenGovernance.listingFeeToken()).to.eq(ethers.constants.AddressZero);

        await tokenGovernance.connect(zkSyncGovernor).setListingFeeToken(paymentToken.address, LISTING_FEE);
    });

    it('Set listing price', async () => {
        await expect(tokenGovernance.connect(userWallet).setListingFee(2)).to.be.revertedWith(REQUIRE_ZKSYNC_GOVERNOR);

        await tokenGovernance.connect(zkSyncGovernor).setListingFee(2);
        expect(await tokenGovernance.listingFee()).to.eq(2);

        await tokenGovernance.connect(zkSyncGovernor).setListingFee(LISTING_FEE);
    });

    it('Add token lister', async () => {
        await expect(tokenGovernance.connect(userWallet).setLister(tokenLister.address, true)).to.be.revertedWith(
            REQUIRE_ZKSYNC_GOVERNOR
        );

        await expect(tokenGovernance.connect(zkSyncGovernor).setLister(tokenLister.address, true))
            .to.emit(tokenGovernance, 'TokenListerUpdate')
            .withArgs(tokenLister.address, true);
    });

    it('Set listing cap', async () => {
        await expect(tokenGovernance.connect(userWallet).setListingCap(MAX_TOKEN + 1)).to.be.revertedWith(
            REQUIRE_ZKSYNC_GOVERNOR
        );

        await tokenGovernance.connect(zkSyncGovernor).setListingCap(MAX_TOKEN + 1);

        expect(await tokenGovernance.listingCap()).to.eq(MAX_TOKEN + 1);
    });

    it('Set treasury', async () => {
        await expect(tokenGovernance.connect(userWallet).setTreasury(ethers.constants.AddressZero)).to.be.revertedWith(
            REQUIRE_ZKSYNC_GOVERNOR
        );

        await tokenGovernance.connect(zkSyncGovernor).setTreasury(ethers.constants.AddressZero);

        expect(await tokenGovernance.treasury()).to.eq(ethers.constants.AddressZero);

        await tokenGovernance.connect(zkSyncGovernor).setTreasury(treasury.address);
    });

    it('New lister can list tokens for free', async () => {
        const newTokenId = (await zkSyncGovernance.totalTokens()) + 1;
        await expect(tokenGovernance.connect(tokenLister).addToken(ERC20_ADDRESS_4))
            .to.emit(zkSyncGovernance, 'NewToken')
            .withArgs(ERC20_ADDRESS_4, newTokenId);
    });
});
