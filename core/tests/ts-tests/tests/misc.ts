import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber, ethers } from 'ethers';
import { SignedTransaction, TxEthSignature } from 'zksync/build/types';
import { submitSignedTransactionsBatch } from 'zksync/build/wallet';
import { MAX_TIMESTAMP } from 'zksync/build/utils';
import { Transfer, Withdraw } from 'zksync/build/types';
import {
    serializeAccountId,
    serializeAddress,
    serializeAmountFull,
    serializeAmountPacked,
    serializeFeePacked,
    serializeNonce,
    serializeTimestamp,
    numberToBytesBE
} from 'zksync/build/utils';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testWrongSignature(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testMultipleBatchSigners(wallets: Wallet[], token: TokenLike, amount: BigNumber): Promise<void>;
        testMultipleWalletsWrongSignature(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testBackwardCompatibleEthMessages(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
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
    const messages: string[] = [];
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
        const transferArgs = {
            to: receiver.address(),
            token,
            amount,
            fee,
            nonce,
            validFrom: 0,
            validUntil: MAX_TIMESTAMP
        };
        const transfer = await sender.getTransfer(transferArgs);
        batch.push({ tx: transfer });

        const messagePart = await sender.getTransferEthMessagePart(transferArgs);
        messages.push(`From: ${sender.address().toLowerCase()}\n${messagePart}\nNonce: ${nonce}`);
    }

    const message = messages.join('\n\n');
    // For every sender there's corresponding signature, otherwise, batch verification would fail.
    const ethSignatures: TxEthSignature[] = [];
    for (let i = 0; i < wallets.length - 1; ++i) {
        ethSignatures.push(await wallets[i].getEthMessageSignature(message));
    }

    const senderBefore = await batchSender.getBalance(token);
    const handles = await submitSignedTransactionsBatch(batchSender.provider, batch, ethSignatures);
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
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
    const _transfer1 = {
        to: to.address(),
        token,
        amount,
        fee: 0,
        nonce: await from.getNonce(),
        validFrom: 0,
        validUntil: MAX_TIMESTAMP
    };
    const _transfer2 = {
        to: from.address(),
        token,
        amount,
        fee,
        nonce: await to.getNonce(),
        validFrom: 0,
        validUntil: MAX_TIMESTAMP
    };
    const transfer1 = await from.getTransfer(_transfer1);
    const transfer2 = await to.getTransfer(_transfer2);
    // transfer1 and transfer2 are from different wallets.
    const batch: SignedTransaction[] = [{ tx: transfer1 }, { tx: transfer2 }];

    const message = `From: ${from.address().toLowerCase()}\n${from.getTransferEthMessagePart(_transfer1)}\nNonce: ${
        _transfer1.nonce
    }\n\nFrom: ${to.address().toLowerCase()}\n${to.getTransferEthMessagePart(_transfer2)}\nNonce: ${_transfer1.nonce}`;
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

// Checks that old formatted 2FA messages are supported.
// The first transaction in a batch is a transfer, the second is a withdraw.
// Signed by both senders.
Tester.prototype.testBackwardCompatibleEthMessages = async function (
    from: Wallet,
    to: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const totalFee = await this.syncProvider.getTransactionsBatchFee(
        ['Transfer', 'Withdraw'],
        [to.address(), to.address()],
        token
    );
    // Transfer
    const transferNonce = await from.getNonce();
    const _transfer = {
        to: to.address(),
        token,
        amount,
        fee: totalFee,
        nonce: transferNonce,
        validFrom: 0,
        validUntil: MAX_TIMESTAMP
    };
    const transfer = await from.getTransfer(_transfer);
    // Resolve all the information needed for human-readable message.
    const stringAmount = from.provider.tokenSet.formatToken(_transfer.token, transfer.amount);
    let stringFee = from.provider.tokenSet.formatToken(_transfer.token, transfer.fee);
    const stringToken = from.provider.tokenSet.resolveTokenSymbol(_transfer.token);
    const transferMessage =
        `Transfer ${stringAmount} ${stringToken}\n` +
        `To: ${transfer.to.toLowerCase()}\n` +
        `Nonce: ${transfer.nonce}\n` +
        `Fee: ${stringFee} ${stringToken}\n` +
        `Account Id: ${transfer.accountId}`;
    const signedTransfer = { tx: transfer, ethereumSignature: await from.getEthMessageSignature(transferMessage) }; // Transfer

    // Withdraw
    const nonce = await to.getNonce();
    const _withdraw = {
        ethAddress: to.address(),
        token,
        amount,
        fee: 0,
        nonce,
        validFrom: 0,
        validUntil: MAX_TIMESTAMP
    };
    const withdraw = await to.getWithdrawFromSyncToEthereum(_withdraw);
    stringFee = from.provider.tokenSet.formatToken(_transfer.token, 0);
    const withdrawMessage =
        `Withdraw ${stringAmount} ${stringToken}\n` +
        `To: ${_withdraw.ethAddress.toLowerCase()}\n` +
        `Nonce: ${withdraw.nonce}\n` +
        `Fee: ${stringFee} ${stringToken}\n` +
        `Account Id: ${withdraw.accountId}`;
    const signedWithdraw = { tx: withdraw, ethereumSignature: await to.getEthMessageSignature(withdrawMessage) }; // Withdraw

    const batch = [signedTransfer, signedWithdraw];

    // The message is keccak256(batchBytes).
    // Transactions are serialized in the old format, the server will take this into account.
    const transferBytes = serializeOldTransfer(transfer);
    const withdrawBytes = serializeOldWithdraw(withdraw);
    const batchBytes = ethers.utils.concat([transferBytes, withdrawBytes]);
    const batchHash = ethers.utils.keccak256(batchBytes).slice(2);
    const message = Uint8Array.from(Buffer.from(batchHash, 'hex'));

    // Both wallets sign it.
    const ethSignatures = [await to.getEthMessageSignature(message), await from.getEthMessageSignature(message)];

    const handles = await submitSignedTransactionsBatch(to.provider, batch, ethSignatures);
    // We only expect that API doesn't reject this batch due to Eth signature error.
    await Promise.all(handles.map((handle) => handle.awaitReceipt()));
    this.runningFee = this.runningFee.add(totalFee);
};

export function serializeOldTransfer(transfer: Transfer): Uint8Array {
    const type = new Uint8Array([5]); // tx type
    const accountId = serializeAccountId(transfer.accountId);
    const from = serializeAddress(transfer.from);
    const to = serializeAddress(transfer.to);
    const token = numberToBytesBE(transfer.token, 2);
    const amount = serializeAmountPacked(transfer.amount);
    const fee = serializeFeePacked(transfer.fee);
    const nonce = serializeNonce(transfer.nonce);
    const validFrom = serializeTimestamp(transfer.validFrom);
    const validUntil = serializeTimestamp(transfer.validUntil);
    return ethers.utils.concat([type, accountId, from, to, token, amount, fee, nonce, validFrom, validUntil]);
}

export function serializeOldWithdraw(withdraw: Withdraw): Uint8Array {
    const type = new Uint8Array([3]);
    const accountId = serializeAccountId(withdraw.accountId);
    const accountBytes = serializeAddress(withdraw.from);
    const ethAddressBytes = serializeAddress(withdraw.to);
    const tokenIdBytes = numberToBytesBE(withdraw.token, 2);
    const amountBytes = serializeAmountFull(withdraw.amount);
    const feeBytes = serializeFeePacked(withdraw.fee);
    const nonceBytes = serializeNonce(withdraw.nonce);
    const validFrom = serializeTimestamp(withdraw.validFrom);
    const validUntil = serializeTimestamp(withdraw.validUntil);
    return ethers.utils.concat([
        type,
        accountId,
        accountBytes,
        ethAddressBytes,
        tokenIdBytes,
        amountBytes,
        feeBytes,
        nonceBytes,
        validFrom,
        validUntil
    ]);
}
