import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { utils } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testMintNFT(
            wallet: Wallet,
            receiver: Wallet,
            feeToken: TokenLike,

            waitVerified?: boolean
        ): Promise<types.NFT>;
    }
}

Tester.prototype.testMintNFT = async function (
    wallet: Wallet,
    receiver: Wallet,
    feeToken: TokenLike,
    waitVerified?: boolean
) {
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
    let receipt;
    if (waitVerified === true) {
        receipt = await handle.awaitVerifyReceipt();
    } else {
        receipt = await handle.awaitReceipt();
    }

    expect(receipt.success, `Mint NFT failed with a reason: ${receipt.failReason}`).to.be.true;

    const balanceAfter = await wallet.getBalance(feeToken);

    expect(balanceBefore.sub(balanceAfter).eq(fee), 'Wrong amount in wallet after withdraw').to.be.true;
    const state = await receiver.getAccountState();
    const nft: any = Object.values(state.committed.nfts)[0];
    expect(nft !== undefined);
    expect(nft.contentHash).eq(utils.hexlify(contentHash));

    this.runningFee = this.runningFee.add(fee);
    return nft;
};
