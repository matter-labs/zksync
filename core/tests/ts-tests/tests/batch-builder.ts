import { Tester, expectThrow } from './tester';
import { expect } from 'chai';
import { Wallet, types, wallet } from 'zksync';
import { BigNumber } from 'ethers';

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
        testBatchBuilderGenericUsage(
            from: Wallet,
            to: Wallet,
            target: Wallet,
            token: TokenLike,
            amount: BigNumber
        ): Promise<void>;
    }
}

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
    sender: Wallet,
    token: TokenLike,
    amount: BigNumber,
    onchain: boolean
) {
    if (onchain) {
        const handle = await sender.onchainAuthSigningKey();
        await handle.wait();
        expect(await sender.isOnchainAuthSigningKeySet(), 'ChangePubKey is unset onchain').to.be.true;
    }

    const batch = await sender
        .batchBuilder()
        .addChangePubKey({ feeToken: token, ethAuthType: onchain ? 'Onchain' : 'ECDSA' })
        .addWithdraw({ ethAddress: sender.address(), token, amount })
        .build(token);
    const totalFee = batch.totalFee.get(token)!;

    const balanceBefore = await sender.getBalance(token);
    const handles = await wallet.submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    expect(await sender.isSigningKeySet(), 'ChangePubKey failed').to.be.true;
    const balanceAfter = await sender.getBalance(token);
    expect(balanceBefore.sub(balanceAfter).eq(amount.add(totalFee)), 'Wrong amount in wallet after withdraw').to.be
        .true;
    this.runningFee = this.runningFee.add(totalFee);
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
    const totalFee = batch.totalFee.get(token)!;

    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);
    const handles = await wallet.submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    expect(senderBefore.sub(senderAfter).eq(amount.mul(2).add(totalFee)), 'Batched transfer failed').to.be.true;
    expect(receiverAfter.sub(receiverBefore).eq(amount.mul(2)), 'Batched transfer failed').to.be.true;
    this.runningFee = this.runningFee.add(totalFee);
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

    const totalFee = batch.totalFee.get(feeToken)!;

    const senderBeforeFeeToken = await sender.getBalance(feeToken);
    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);
    const handles = await wallet.submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    const senderAfterFeeToken = await sender.getBalance(feeToken);
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    expect(senderBeforeFeeToken.sub(senderAfterFeeToken).eq(totalFee), 'Paying in another token failed').to.be.true;
    expect(senderBefore.sub(senderAfter).eq(amount.mul(2)), 'Batched transfer failed').to.be.true;
    expect(receiverAfter.sub(receiverBefore).eq(amount.mul(2)), 'Batched transfer failed').to.be.true;
    // Do not increase running fee, feeToken is different.
};

Tester.prototype.testBatchBuilderGenericUsage = async function (
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

    const totalFee = batch.totalFee.get(token)!;

    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);
    const handles = await wallet.submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    const targetBalance = await target.getBalance(token);

    expect(senderBefore.sub(senderAfter).eq(amount.mul(2).add(totalFee)), 'Batch execution failed').to.be.true;
    expect(receiverAfter.sub(receiverBefore).eq(amount), 'Transfer failed').to.be.true;
    expect(targetBalance.isZero(), 'Forced exit failed');
    this.runningFee = this.runningFee.add(totalFee);
};
