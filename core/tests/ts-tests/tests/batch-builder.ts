import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber } from 'ethers';
import { submitSignedTransactionsBatch } from 'zksync/build/wallet';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testBatchBuilderInvalidUsage(wallet: Wallet, feeToken: TokenLike): Promise<void>;
        testBatchBuilderChangePubKey(
            wallet: Wallet,
            token: TokenLike,
            amount: BigNumber,
            onchain: boolean
        ): Promise<void>;
        testBatchBuilderTransfers(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testBatchBuilderPayInDifferentToken(
            from: Wallet,
            to: Wallet,
            token: TokenLike,
            feeToken: TokenLike,
            amount: BigNumber
        ): Promise<void>;
        testBatchBuilderGenerisUsage(
            from: Wallet,
            to: Wallet,
            target: Wallet,
            token: TokenLike,
            amount: BigNumber
        ): Promise<void>;
    }
}

const expectThrow = async (promise: Promise<any>, message: String) => {
    let error = null;
    try {
        await promise;
    } catch (err) {
        error = err;
    }
    expect(error).to.be.an('Error');
    expect(error.message).to.equal(message);
};

Tester.prototype.testBatchBuilderInvalidUsage = async function (wallet: Wallet, feeToken: TokenLike) {
    // Empty batch.
    await expectThrow(wallet.batchBuilder().build(), 'Transaction batch cannot be empty');
    // Specify both transaction fee and the paying token.
    await expectThrow(
        wallet
            .batchBuilder()
            .addChangePubKey({ feeToken, ethAuthType: 'ECDSA' })
            .addTransfer({ to: wallet.address(), token: feeToken, amount: 0, fee: 999 })
            .build(feeToken),
        'Fees are expected to be zero'
    );
};

// Set signing key and perform a withdraw in one batch.
Tester.prototype.testBatchBuilderChangePubKey = async function (
    wallet: Wallet,
    token: TokenLike,
    amount: BigNumber,
    onchain: boolean
) {
    if (onchain) {
        const handle = await wallet.onchainAuthSigningKey();
        await handle.wait();
        expect(await wallet.isOnchainAuthSigningKeySet(), 'ChangePubKey is unset onchain').to.be.true;
    }

    const batch = await wallet
        .batchBuilder()
        .addChangePubKey({ feeToken: token, ethAuthType: onchain ? 'Onchain' : 'ECDSA' })
        .addWithdraw({ ethAddress: wallet.address(), token, amount })
        .build(token);

    const balanceBefore = await wallet.getBalance(token);
    const handles = await submitSignedTransactionsBatch(wallet.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitVerifyReceipt()));
    expect(await wallet.isSigningKeySet(), 'ChangePubKey failed').to.be.true;
    const balanceAfter = await wallet.getBalance(token);
    expect(balanceBefore.sub(balanceAfter).eq(amount.add(batch.totalFee)), 'Wrong amount in wallet after withdraw').to
        .be.true;
    this.runningFee = this.runningFee.add(batch.totalFee);
};

// Copy-paste of multiTransfer test, batchBuilder must work the same way.
Tester.prototype.testBatchBuilderTransfers = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const batch = await sender
        .batchBuilder()
        .addTransfer({ to: receiver.address(), token, amount })
        .addTransfer({ to: receiver.address(), token, amount })
        .build(token);

    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);
    const handles = await submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    expect(senderBefore.sub(senderAfter).eq(amount.mul(2).add(batch.totalFee)), 'Batched transfer failed').to.be.true;
    expect(receiverAfter.sub(receiverBefore).eq(amount.mul(2)), 'Batched transfer failed').to.be.true;
    this.runningFee = this.runningFee.add(batch.totalFee);
};

// The same as multiTransfer, but we specify different token to pay with, so the third transfer to self is created.
Tester.prototype.testBatchBuilderPayInDifferentToken = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    feeToken: TokenLike,
    amount: BigNumber
) {
    expect(token != feeToken, 'token and feeToken are expected to be different').to.be.true;

    const batch = await sender
        .batchBuilder()
        .addTransfer({ to: receiver.address(), token, amount })
        .addTransfer({ to: receiver.address(), token, amount })
        .build(feeToken);

    expect(batch.txs.length == 3, 'Wrong batch length').to.be.true;

    const senderBeforeFeeToken = await sender.getBalance(feeToken);
    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);
    const handles = await submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    const senderAfterFeeToken = await sender.getBalance(feeToken);
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    expect(senderBeforeFeeToken.sub(senderAfterFeeToken).eq(batch.totalFee), 'Paying in another token failed').to.be
        .true;
    expect(senderBefore.sub(senderAfter).eq(amount.mul(2)), 'Batched transfer failed').to.be.true;
    expect(receiverAfter.sub(receiverBefore).eq(amount.mul(2)), 'Batched transfer failed').to.be.true;
    // Do not increase running fee, feeToken is different.
};

Tester.prototype.testBatchBuilderGenerisUsage = async function (
    sender: Wallet,
    receiver: Wallet,
    target: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const batch = await sender
        .batchBuilder()
        .addTransfer({ to: receiver.address(), token, amount })
        .addWithdraw({ ethAddress: sender.address(), token, amount })
        .addForcedExit({ target: target.address(), token })
        .build(token);

    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);
    const handles = await submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitVerifyReceipt()));
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    const targetBalance = await target.getBalance(token);

    expect(senderBefore.sub(senderAfter).eq(amount.mul(2).add(batch.totalFee)), 'Batch execution failed').to.be.true;
    expect(receiverAfter.sub(receiverBefore).eq(amount), 'Transfer failed').to.be.true;
    expect(targetBalance.isZero(), 'Forced exit failed');
    this.runningFee = this.runningFee.add(batch.totalFee);
};
