import { Tester, expectThrow } from './tester';
import { expect } from 'chai';
import { Wallet, types, wallet } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testBatchBuilderInvalidUsage(wallet: Wallet, unlockedWallet: Wallet, feeToken: TokenLike): Promise<void>;
        testBatchBuilderChangePubKey(
            wallet: Wallet,
            token: TokenLike,
            amount: BigNumber,
            onchain: boolean
        ): Promise<void>;
        testBatchBuilderSignedChangePubKey(sender: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
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
        testBatchBuilderNFT(from: Wallet, to: Wallet, feeToken: TokenLike): Promise<void>;
    }
}

Tester.prototype.testBatchBuilderInvalidUsage = async function (
    wallet: Wallet,
    unlockedWallet: Wallet,
    feeToken: TokenLike
) {
    // Empty batch.
    await expectThrow(wallet.batchBuilder().build(), 'Transaction batch cannot be empty');
    // Specify both transaction fee and the paying token.
    await expectThrow(
        wallet
            .batchBuilder()
            .addChangePubKey({ feeToken, ethAuthType: 'ECDSA' })
            .addTransfer({ to: wallet.address(), token: feeToken, amount: 0, fee: 999 })
            .build(feeToken),
        'All transactions are expected to be unsigned with zero fees'
    );
    await expectThrow(
        unlockedWallet.batchBuilder().addChangePubKey({ feeToken, ethAuthType: 'ECDSA' }).build(feeToken),
        'Current signing key is already set'
    );
};

// Set signing key and perform a withdraw in one batch.
Tester.prototype.testBatchBuilderChangePubKey = async function (
    sender: Wallet,
    token: TokenLike,
    amount: BigNumber,
    onchain: boolean
) {
    expect(await sender.isSigningKeySet(), 'Signing key is already set').to.be.false;

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

// Build a batch with a signed ChangePubKey transaction.
Tester.prototype.testBatchBuilderSignedChangePubKey = async function (
    sender: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    expect(await sender.isSigningKeySet(), 'Signing key is already set').to.be.false;

    const nonce = await sender.getNonce();
    const ethAuthType: types.ChangePubkeyTypes = 'ECDSALegacyMessage';
    const changePubKeyType = {
        ChangePubKey: {
            onchainPubkeyAuth: false
        }
    };
    // Sign the transaction before constructing the batch.
    const signedTx = await sender.signSetSigningKey({ feeToken: token, fee: 0, nonce, ethAuthType });
    // BatchBuilder won't be able to set the fee since we have a signed transaction, obtain it ourselves.
    const fee = await sender.provider.getTransactionsBatchFee(
        ['Transfer', 'Transfer', changePubKeyType],
        Array(3).fill(sender.address()),
        token
    );

    const batchBuilder = sender
        .batchBuilder()
        .addChangePubKey(signedTx)
        .addTransfer({ to: sender.address(), token, amount });
    // Should throw if we try to set a fee token with a signed transaction.
    await expectThrow(batchBuilder.build(token), 'All transactions are expected to be unsigned with zero fees');
    // Transfer to self to pay the fee.
    batchBuilder.addTransfer({ to: sender.address(), token, amount, fee });

    const batch = await batchBuilder.build();
    const totalFee = batch.totalFee.get(token)!;
    // Should be equal.
    expect(fee.eq(totalFee), 'Wrong caclucated fee').to.be.true;

    const balanceBefore = await sender.getBalance(token);
    const handles = await wallet.submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    expect(await sender.isSigningKeySet(), 'ChangePubKey failed').to.be.true;
    const balanceAfter = await sender.getBalance(token);
    expect(balanceBefore.sub(balanceAfter).eq(totalFee), 'Wrong amount in wallet after withdraw').to.be.true;
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

Tester.prototype.testBatchBuilderNFT = async function (from: Wallet, to: Wallet, feeToken: TokenLike) {
    const mint_batch = await from
        .batchBuilder()
        .addMintNFT({ recipient: to.address(), contentHash: '0x' + '2'.padStart(64, '0'), feeToken })
        .addMintNFT({ recipient: to.address(), contentHash: '0x' + '3'.padStart(64, '0'), feeToken })
        .build(feeToken);

    const totalMintFee = mint_batch.totalFee.get(feeToken)!;

    const mint_handles = await wallet.submitSignedTransactionsBatch(from.provider, mint_batch.txs, [
        mint_batch.signature
    ]);
    await Promise.all(mint_handles.map((handle) => handle.awaitVerifyReceipt()));

    const state_after_mint = await to.getAccountState();
    let nft1: any = Object.values(state_after_mint.verified.nfts)[0];
    let nft2: any = Object.values(state_after_mint.verified.nfts)[1];

    const balanceAfterMint1 = await to.getNFT(nft1.id);
    const balanceAfterMint2 = await to.getNFT(nft2.id);
    expect(balanceAfterMint1.id == nft1.id, 'Account does not have any NFT after two mintNFT txs').to.be.true;
    expect(balanceAfterMint2.id == nft2.id, 'Account has only one NFT after two mintNFT txs').to.be.true;

    this.runningFee = this.runningFee.add(totalMintFee);

    const withdraw_batch = await to
        .batchBuilder()
        .addWithdrawNFT({ to: to.address(), token: nft1.id, feeToken })
        .addWithdrawNFT({ to: to.address(), token: nft2.id, feeToken })
        .build(feeToken);

    const totalWithdrawFee = withdraw_batch.totalFee.get(feeToken)!;

    const withdraw_handles = await wallet.submitSignedTransactionsBatch(to.provider, withdraw_batch.txs, [
        withdraw_batch.signature
    ]);
    await Promise.all(withdraw_handles.map((handle) => handle.awaitReceipt()));

    const balanceAfterWithdraw1 = await to.getNFT(nft1.id);
    const balanceAfterWithdraw2 = await to.getNFT(nft2.id);
    expect(balanceAfterWithdraw1 === undefined, 'Account has NFT after two withdrawNFT txs').to.be.true;
    expect(balanceAfterWithdraw2 === undefined, 'Account has NFT after two withdrawNFT txs').to.be.true;

    this.runningFee = this.runningFee.add(totalWithdrawFee);
};
