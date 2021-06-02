import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, utils, wallet } from 'zksync';
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
        testSwapBatch(
            walletA: Wallet,
            walletB: Wallet,
            walletC: Wallet,
            tokenA: TokenLike,
            tokenB: TokenLike,
            amount: BigNumber
        ): Promise<void>;
        testSwapNFT(walletA: Wallet, walletB: Wallet, token: TokenLike, nft: number, amount: BigNumber): Promise<void>;
    }
}

Tester.prototype.testSwapNFT = async function (
    walletA: Wallet,
    walletB: Wallet,
    token: TokenLike,
    nft: number,
    amount: BigNumber
) {
    const { totalFee: fee } = await this.syncProvider.getTransactionFee('Swap', walletA.address(), token);
    expect(await walletB.getNFT(nft), 'wallet does not own an NFT').to.exist;

    const orderA = await walletA.getOrder({
        tokenSell: token,
        tokenBuy: nft,
        amount,
        ratio: utils.weiRatio({
            [token]: amount,
            [nft]: 1
        })
    });

    const orderB = await walletB.getOrder({
        tokenSell: nft,
        tokenBuy: token,
        amount: 1,
        ratio: utils.weiRatio({
            [token]: amount,
            [nft]: 1
        })
    });

    const swap = await walletA.syncSwap({
        orders: [orderA, orderB],
        feeToken: token,
        fee
    });

    const receipt = await swap.awaitReceipt();
    expect(receipt.success, `Swap transaction failed with a reason: ${receipt.failReason}`).to.be.true;
    expect(await walletA.getNFT(nft), 'NFT was not swapped').to.exist;
    expect(await walletB.getNFT(nft), 'NFT is present even after swap').to.not.exist;

    this.runningFee = this.runningFee.add(fee);
};

Tester.prototype.testSwap = async function (
    walletA: Wallet,
    walletB: Wallet,
    tokenA: TokenLike,
    tokenB: TokenLike,
    amount: BigNumber
) {
    const { totalFee: fee } = await this.syncProvider.getTransactionFee('Swap', walletA.address(), tokenA);
    const stateABefore = (await this.syncProvider.getState(walletA.address())).committed;
    const stateBBefore = (await this.syncProvider.getState(walletB.address())).committed;

    const orderA = await walletA.getOrder({
        tokenSell: tokenA,
        tokenBuy: tokenB,
        amount,
        ratio: utils.weiRatio({
            [tokenA]: 1,
            [tokenB]: 2
        })
    });

    const orderB = await walletB.getOrder({
        tokenSell: tokenB,
        tokenBuy: tokenA,
        amount: amount.mul(2),
        ratio: utils.weiRatio({
            [tokenA]: 1,
            [tokenB]: 2
        })
    });

    const swap = await walletA.syncSwap({
        orders: [orderA, orderB],
        feeToken: tokenA,
        fee
    });

    const receipt = await swap.awaitReceipt();
    expect(receipt.success, `Swap transaction failed with a reason: ${receipt.failReason}`).to.be.true;

    const stateAAfter = (await this.syncProvider.getState(walletA.address())).committed;
    const stateBAfter = (await this.syncProvider.getState(walletB.address())).committed;

    const diffA = {
        tokenA: BigNumber.from(stateABefore.balances[tokenA] || 0).sub(stateAAfter.balances[tokenA] || 0),
        tokenB: BigNumber.from(stateAAfter.balances[tokenB] || 0).sub(stateABefore.balances[tokenB] || 0),
        nonce: stateAAfter.nonce - stateABefore.nonce
    };
    const diffB = {
        tokenB: BigNumber.from(stateBBefore.balances[tokenB] || 0).sub(stateBAfter.balances[tokenB] || 0),
        tokenA: BigNumber.from(stateBAfter.balances[tokenA] || 0).sub(stateBBefore.balances[tokenA] || 0),
        nonce: stateBAfter.nonce - stateBBefore.nonce
    };

    expect(diffA.tokenA.eq(amount.add(fee)), 'Wrong amount after swap (walletA, tokenA)').to.be.true;
    expect(diffA.tokenB.eq(amount.mul(2)), 'Wrong amount after swap (walletA, tokenB)').to.be.true;
    expect(diffB.tokenB.eq(amount.mul(2)), 'Wrong amount after swap (walletB, tokenB)').to.be.true;
    expect(diffB.tokenA.eq(amount), 'Wrong amount after swap (walletB, tokenA)').to.be.true;
    expect(diffA.nonce, 'Wrong nonce after swap (wallet A)').to.eq(1);
    expect(diffB.nonce, 'Wrong nonce after swap (wallet B)').to.eq(1);

    this.runningFee = this.runningFee.add(fee);
};

Tester.prototype.testSwapBatch = async function (
    walletA: Wallet,
    walletB: Wallet,
    walletC: Wallet,
    tokenA: TokenLike,
    tokenB: TokenLike,
    amount: BigNumber
) {
    const nonceBefore = await walletA.getNonce();

    // these are limit orders, so they can be reused
    const orderA = await walletA.getLimitOrder({
        tokenSell: tokenA,
        tokenBuy: tokenB,
        ratio: utils.weiRatio({
            [tokenA]: 2,
            [tokenB]: 5
        })
    });

    const orderB = await walletB.getLimitOrder({
        tokenSell: tokenB,
        tokenBuy: tokenA,
        ratio: utils.weiRatio({
            [tokenA]: 1,
            [tokenB]: 4
        })
    });

    const batch = await walletC
        .batchBuilder()
        .addSwap({
            orders: [orderA, orderB],
            amounts: [amount.div(5), amount.div(2)],
            feeToken: tokenA
        })
        .addSwap({
            orders: [orderB, orderA],
            amounts: [amount, amount.div(4)],
            feeToken: tokenA
        })
        .build(tokenA);

    const handles = await wallet.submitSignedTransactionsBatch(this.syncProvider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));

    const nonceAfter = await walletA.getNonce();
    expect(nonceAfter, 'Nonce should not increase after limit order is partially filled').to.eq(nonceBefore);

    this.runningFee = this.runningFee.add(batch.totalFee.get(tokenA) || 0);
};
