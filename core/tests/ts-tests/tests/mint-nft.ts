import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { utils } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testMintNFT(wallet: Wallet, receiver: Wallet, feeToken: TokenLike): Promise<types.NFT>;
    }
}

Tester.prototype.testMintNFT = async function (wallet: Wallet, receiver: Wallet, feeToken: TokenLike) {
    const type = 'MintNFT';
    const contentHash = utils.randomBytes(32);
    let { totalFee: fee } = await this.syncProvider.getTransactionFee(type, wallet.address(), feeToken);

    const handle = await wallet.mintNFT({
        recipient: receiver.address(),
        contentHash,
        feeToken,
        fee
    });

    const balanceBefore = await wallet.getBalance(feeToken);
    const receipt = await handle.awaitReceipt();
    expect(receipt.success, `Mint NFT failed with a reason: ${receipt.failReason}`).to.be.true;

    const balanceAfter = await wallet.getBalance(feeToken);

    expect(balanceBefore.sub(balanceAfter).eq(fee), 'Wrong amount in wallet after withdraw').to.be.true;
    const state = await receiver.getAccountState();
    const nft = Object.values(state.committed.nfts)[0];
    expect(nft.contentHash).eq(utils.hexlify(contentHash));

    this.runningFee = this.runningFee.add(fee);
    return nft;
};
