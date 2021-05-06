import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, utils } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testSwap(
            walletA: Wallet,
            walletB: Wallet,
            tokenA: TokenLike,
            tokenB: TokenLike,
            amount: BigNumber
        ): Promise<void>;
    }
}

Tester.prototype.testSwap = async function (
    walletA: Wallet,
    walletB: Wallet,
    tokenA: TokenLike,
    tokenB: TokenLike,
    amount: BigNumber
) {
    const { totalFee: fee } = await this.syncProvider.getTransactionFee('Swap', walletA.address(), tokenA);
    const balanceABefore = (await this.syncProvider.getState(walletA.address())).committed.balances;
    const balanceBBefore = (await this.syncProvider.getState(walletB.address())).committed.balances;

    const orderA = await walletA.getOrder({
        tokenSell: tokenA,
        tokenBuy: tokenB,
        amount,
        price: utils.price({
            sellPrice: 1,
            buyPrice: 2
        })
    });

    const orderB = await walletB.getOrder({
        tokenSell: tokenB,
        tokenBuy: tokenA,
        amount: amount.mul(2),
        price: utils.price({
            sellPrice: 2,
            buyPrice: 1
        })
    });

    const swap = await walletA.syncSwap({
        orders: [orderA, orderB],
        feeToken: tokenA,
        fee
    });

    const receipt = await swap.awaitReceipt();
    expect(receipt.success, `Swap transaction failed with a reason: ${receipt.failReason}`).to.be.true;

    const balanceAAfter = (await this.syncProvider.getState(walletA.address())).committed.balances;
    const balanceBAfter = (await this.syncProvider.getState(walletB.address())).committed.balances;

    const diffA = {
        tokenA: BigNumber.from(balanceABefore[tokenA] || 0).sub(balanceAAfter[tokenA] || 0),
        tokenB: BigNumber.from(balanceAAfter[tokenB] || 0).sub(balanceABefore[tokenB] || 0)
    };
    const diffB = {
        tokenB: BigNumber.from(balanceBBefore[tokenB] || 0).sub(balanceBAfter[tokenB] || 0),
        tokenA: BigNumber.from(balanceBAfter[tokenA] || 0).sub(balanceBBefore[tokenA] || 0)
    };

    expect(diffA.tokenA.eq(amount.add(fee)), 'Wrong amount after swap (walletA, tokenA)').to.be.true;
    expect(diffA.tokenB.eq(amount.mul(2)), 'Wrong amount after swap (walletA, tokenB)').to.be.true;
    expect(diffB.tokenB.eq(amount.mul(2)), 'Wrong amount after swap (walletB, tokenB)').to.be.true;
    expect(diffB.tokenA.eq(amount), 'Wrong amount in swap (walletB, tokenA)').to.be.true;

    this.runningFee = this.runningFee.add(fee);
};
