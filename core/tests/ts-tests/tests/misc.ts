import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber, ethers } from 'ethers';
import { SignedTransaction, TxEthSignature } from 'zksync/build/types';
import { serializeTx } from 'zksync/build/utils';
import { submitSignedTransactionsBatch } from 'zksync/build/wallet';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testWrongSignature(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testMultipleBatchSigners(wallets: Wallet[], token: TokenLike, amount: BigNumber): Promise<void>;
        testMultipleWalletsWrongSignature(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
    }
}

Tester.prototype.testWrongSignature = async function (from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber) {
    const signedTransfer = await from.signSyncTransfer({
        to: to.address(),
        token: token,
        amount,
        fee: amount.div(2),
        nonce: await from.getNonce()
    });

    const ETH_SIGNATURE_LENGTH_PREFIXED = 132;
    const fakeEthSignature: types.TxEthSignature = {
        signature: '0x'.padEnd(ETH_SIGNATURE_LENGTH_PREFIXED, '0'),
        type: 'EthereumSignature'
    };

    let thrown = true;
    try {
        await from.provider.submitTx(signedTransfer.tx, fakeEthSignature);
        thrown = false; // this line should be unreachable
    } catch (e) {
        expect(e.jrpcError.message).to.equal('Eth signature is incorrect');
    }
    expect(thrown, 'Sending tx with incorrect ETH signature must throw').to.be.true;

    const { totalFee } = await this.syncProvider.getTransactionFee('Withdraw', from.address(), token);
    const signedWithdraw = await from.signWithdrawFromSyncToEthereum({
        ethAddress: from.address(),
        token: token,
        amount: amount.div(2),
        fee: totalFee,
        nonce: await from.getNonce()
    });

    thrown = true;
    try {
        await from.provider.submitTx(signedWithdraw.tx, fakeEthSignature);
        thrown = false; // this line should be unreachable
    } catch (e) {
        expect(e.jrpcError.message).to.equal('Eth signature is incorrect');
    }
    expect(thrown, 'Sending tx with incorrect ETH signature must throw').to.be.true;
};

// First, cycle of transfers is created in the following way:
// 1 -> 2 -> 3 -> 1
// This batch is then signed by every wallet, the first wallet gets to pay the whole fee.
Tester.prototype.testMultipleBatchSigners = async function (wallets: Wallet[], token: TokenLike, amount: BigNumber) {
    expect(wallets.length >= 2, 'At least 2 wallets are expected').to.be.true;
    const batch: SignedTransaction[] = [];
    // The first account will send the batch and pay all the fees.
    const batchSender = wallets[0];
    const types = Array(wallets.length).fill('Transfer');
    const addresses = wallets.map((wallet) => wallet.address());
    const totalFee = await batchSender.provider.getTransactionsBatchFee(types, addresses, token);
    // Create cycle of transfers.
    wallets.push(batchSender);
    for (let i = 0, j = i + 1; j < wallets.length; ++i, ++j) {
        const sender = wallets[i];
        const receiver = wallets[j];
        // Fee is zero for all wallets except the one sending this batch.
        const fee = BigNumber.from(i == 0 ? totalFee : 0);
        const nonce = await sender.getNonce();
        const transfer = await sender.getTransfer({ to: receiver.address(), token, amount, fee, nonce });
        batch.push({ tx: transfer });
    }
    // The message is keccak256(batchBytes)
    const batchBytes = ethers.utils.concat(batch.map((signedTx) => serializeTx(signedTx.tx)));
    const batchHash = ethers.utils.keccak256(batchBytes).slice(2);
    const message = Uint8Array.from(Buffer.from(batchHash, 'hex'));
    // For every sender there's corresponding signature, otherwise, batch verification would fail.
    const ethSignatures: TxEthSignature[] = [];
    for (let i = 0; i < wallets.length - 1; ++i) {
        ethSignatures.push(await wallets[i].getEthMessageSignature(message));
    }

    const senderBefore = await batchSender.getBalance(token);
    const handles = await submitSignedTransactionsBatch(batchSender.provider, batch, ethSignatures);
    await Promise.all(handles.map((handle) => handle.awaitVerifyReceipt()));
    const senderAfter = await batchSender.getBalance(token);
    // Sender paid totalFee for this cycle.
    expect(senderBefore.sub(senderAfter).eq(totalFee), 'Batched transfer failed').to.be.true;
    this.runningFee = this.runningFee.add(totalFee);
};

// Include two transfers in one batch, but provide signature only from one sender.
Tester.prototype.testMultipleWalletsWrongSignature = async function (
    from: Wallet,
    to: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const fee = await this.syncProvider.getTransactionsBatchFee(
        ['Transfer', 'Transfer'],
        [from.address(), to.address()],
        token
    );
    const transfer1 = await from.getTransfer({
        to: to.address(),
        token,
        amount,
        fee: 0,
        nonce: await from.getNonce()
    });
    const transfer2 = await to.getTransfer({ to: from.address(), token, amount, fee, nonce: await to.getNonce() });
    // transfer1 and transfer2 are from different wallets.
    const batch: SignedTransaction[] = [{ tx: transfer1 }, { tx: transfer2 }];

    const batchBytes = ethers.utils.concat(batch.map((signedTx) => serializeTx(signedTx.tx)));
    const batchHash = ethers.utils.keccak256(batchBytes).slice(2);
    const message = Uint8Array.from(Buffer.from(batchHash, 'hex'));
    const ethSignature = await from.getEthMessageSignature(message);

    let thrown = true;
    try {
        await submitSignedTransactionsBatch(from.provider, batch, [ethSignature]);
        thrown = false; // this line should be unreachable
    } catch (e) {
        expect(e.jrpcError.message).to.equal('Eth signature is incorrect');
    }
    expect(thrown, 'Sending batch with incorrect ETH signature must throw').to.be.true;
};
