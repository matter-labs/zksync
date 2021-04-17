import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testMintNFT(wallet: Wallet, receiver: Wallet, contentHash: string, feeToken: TokenLike): Promise<void>;
    }
}

Tester.prototype.testMintNFT = async function (
    wallet: Wallet,
    receiver: Wallet,
    contentHash: string,
    feeToken: TokenLike
) {
    const type = 'MintNFT';
    let { totalFee: fee } = await this.syncProvider.getTransactionFee(type, wallet.address(), feeToken);

    const handle = await wallet.mintNFT({
        recipient: receiver.address(),
        contentHash,
        feeToken,
        fee
    });

    const receipt = await handle.awaitReceipt();
    expect(receipt.success, `Mint NFT failed with a reason: ${receipt.failReason}`).to.be.true;

    // const balanceAfter = await wallet.getBalance(token);
    // expect(balanceBefore.sub(balanceAfter).eq(amount.add(fee)), 'Wrong amount in wallet after withdraw').to.be.true;
    // this.runningFee = this.runningFee.add(fee);
    // return handle;
};
