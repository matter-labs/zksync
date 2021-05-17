import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber } from 'ethers';
import { closestPackableTransactionFee } from '../../../../sdk/zksync.js';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testTransfer(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testTransferNFT(from: Wallet, to: Wallet, feeToken: TokenLike): Promise<void>;
        testBatch(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testIgnoredBatch(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testRejectedBatch(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testInvalidFeeBatch(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
    }
}

async function suppress<T>(promise: Promise<T>) {
    try {
        await promise;
    } catch (_) {}
}

Tester.prototype.testTransfer = async function (sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber) {
    const fullFee = await this.syncProvider.getTransactionFee('Transfer', receiver.address(), token);
    const fee = fullFee.totalFee;
    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);

    const handle = await sender.syncTransfer({
        to: receiver.address(),
        token,
        amount,
        fee
    });

    const receipt = await handle.awaitReceipt();
    expect(receipt.success, `Transfer transaction failed with a reason: ${receipt.failReason}`).to.be.true;
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);

    if (sender.address() === receiver.address()) {
        expect(senderBefore.sub(fee).eq(senderAfter), 'Transfer to self failed').to.be.true;
    } else {
        expect(senderBefore.sub(senderAfter).eq(amount.add(fee)), 'Transfer failed (incorrect sender balance)').to.be
            .true;
        expect(receiverAfter.sub(receiverBefore).eq(amount), 'Transfer failed (incorrect receiver balance)').to.be.true;
    }

    this.runningFee = this.runningFee.add(fee);
};

Tester.prototype.testTransferNFT = async function (sender: Wallet, receiver: Wallet, feeToken: TokenLike) {
    const fee = await this.syncProvider.getTransactionsBatchFee(
        ['Transfer', 'Transfer'],
        [receiver.address(), sender.address()],
        feeToken
    );

    const state = await sender.getAccountState();
    const nft = Object.values(state.verified.nfts)[0];
    expect(nft !== undefined);
    const senderBefore = await sender.getNFT(nft.id);
    const receiverBefore = await receiver.getNFT(nft.id);
    const handles = await sender.syncTransferNFT({
        to: receiver.address(),
        feeToken,
        token: nft,
        fee
    });

    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    const senderAfter = await sender.getNFT(nft.id);
    const receiverAfter = await receiver.getNFT(nft.id);
    expect(senderBefore, 'NFT transfer failed').to.exist;
    expect(receiverAfter, 'NFT transfer failed').to.exist;
    expect(senderAfter, 'NFT transfer failed').to.not.exist;
    expect(receiverBefore, 'NFT transfer failed').to.not.exist;
    this.runningFee = this.runningFee.add(fee);
};

Tester.prototype.testBatch = async function (sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber) {
    const fee = await this.syncProvider.getTransactionsBatchFee(
        ['Transfer', 'Transfer'],
        [receiver.address(), receiver.address()],
        token
    );

    const tx_with_fee = {
        to: receiver.address(),
        token,
        amount,
        fee
    };
    const tx_without_fee = {
        to: receiver.address(),
        token,
        amount,
        fee: 0
    };
    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);
    const handles = await sender.syncMultiTransfer([tx_with_fee, tx_without_fee]);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    expect(senderBefore.sub(senderAfter).eq(amount.mul(2).add(fee)), 'Batched transfer failed').to.be.true;
    expect(receiverAfter.sub(receiverBefore).eq(amount.mul(2)), 'Batched transfer failed').to.be.true;
    this.runningFee = this.runningFee.add(fee);
};

Tester.prototype.testIgnoredBatch = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const fee = await this.syncProvider.getTransactionsBatchFee(
        ['Transfer', 'Transfer'],
        [receiver.address(), receiver.address()],
        token
    );

    const tx = {
        to: receiver.address(),
        token,
        amount,
        fee: closestPackableTransactionFee(fee.div(2))
    };

    const senderBefore = await sender.getBalance(token);
    const receiverBefore = await receiver.getBalance(token);
    const handles = await sender.syncMultiTransfer([
        { ...tx },
        // set amount too big
        { ...tx, amount: amount.mul(10 ** 6) }
    ]);

    for (const handle of handles) {
        await suppress(handle.awaitReceipt());
    }

    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);
    expect(senderBefore.eq(senderAfter), 'Wrong batch was not ignored').to.be.true;
    expect(receiverAfter.eq(receiverBefore), 'Wrong batch was not ignored').to.be.true;
};

Tester.prototype.testRejectedBatch = async function (
    sender: Wallet,
    receiver: Wallet,
    token: types.TokenLike,
    amount: BigNumber
) {
    const tx = {
        to: receiver.address(),
        token,
        amount,
        fee: BigNumber.from('0')
    };

    let thrown = true;
    try {
        const handles = await sender.syncMultiTransfer([{ ...tx }, { ...tx }]);
        for (const handle of handles) {
            await handle.awaitVerifyReceipt();
        }
        thrown = false; // this line should be unreachable
    } catch (e) {
        expect(e.jrpcError.message).to.equal('Transactions batch summary fee is too low');
    }
    expect(thrown, 'Batch should have failed').to.be.true;
};

// Checks that the server takes into account all transactions from the batch when calculating
// the fee.
Tester.prototype.testInvalidFeeBatch = async function (
    sender: Wallet,
    receiver: Wallet,
    token: types.TokenLike,
    amount: BigNumber
) {
    // Ignore the second transfer.
    const fee = await this.syncProvider.getTransactionsBatchFee(['Transfer'], [receiver.address()], token);

    const txWithFee = {
        to: receiver.address(),
        token,
        amount,
        fee
    };
    const txWithoutFee = {
        to: receiver.address(),
        token,
        amount,
        fee: 0
    };

    const multiTransfer = [];
    for (let i = 0; i < 10; ++i) {
        multiTransfer.push(txWithoutFee);
    }
    multiTransfer.push(txWithFee);

    let thrown = true;
    try {
        const handles = await sender.syncMultiTransfer(multiTransfer);
        for (const handle of handles) {
            await handle.awaitVerifyReceipt();
        }
        thrown = false; // this line should be unreachable
    } catch (e) {
        expect(e.jrpcError.message).to.equal('Transactions batch summary fee is too low');
    }
    expect(thrown, 'Batch should have failed').to.be.true;
};
