import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber, ethers, Signer } from 'ethers';
import { SignedTransaction, TxEthSignature, Address } from 'zksync/build/types';
import { serializeTx, sleep } from 'zksync/build/utils';
import { submitSignedTransactionsBatch } from 'zksync/build/wallet';
import { MAX_TIMESTAMP } from 'zksync/build/utils';

import { RevertReceiveAccountFactory, RevertTransferERC20Factory } from '../../../../contracts/typechain';
import { waitForOnchainWithdrawal }  from './helpers';

import { withdrawalHelpers } from 'zksync';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testWrongSignature(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testMultipleBatchSigners(wallets: Wallet[], token: TokenLike, amount: BigNumber): Promise<void>;
        testMultipleWalletsWrongSignature(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testRecoverETHWithdrawal(from: Wallet, to: Address, toOwner: Wallet, amount: BigNumber): Promise<void>;
        testRecoverERC20Withdrawal(from: Wallet, to: Address, token: TokenLike, amount: BigNumber): Promise<void>;
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

        const messagePart = sender.getTransferEthMessagePart(transferArgs);
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

Tester.prototype.testRecoverETHWithdrawal = async function (
    from: Wallet,
    to: Address,
    toOwner: Wallet,
    amount: BigNumber
) {
    const revertReceiveContract = RevertReceiveAccountFactory.connect(
        to,
        toOwner.ethSigner
    );

    // Making sure that the withdrawal will be reverted 
    await revertReceiveContract.setRevertReceive(true);

    const balanceBefore = await this.ethProvider.getBalance(to);
    const withdrawTx = await from.withdrawFromSyncToEthereum({
        ethAddress: to,
        token: 'ETH',
        amount
    });
    await withdrawTx.awaitVerifyReceipt();
    
    // Waiting for the withdrawl to be sent onchain
    const withdrawalTxHash = await waitForOnchainWithdrawal(
        this.syncProvider,
        withdrawTx.txHash
    );
    
    // Double-check that zkSync tried to process withdrawal
    expect(withdrawalTxHash, 'Withdrawal was not processed onchain').to.exist;

    // double-check that the withdrawal has indeed failed
    const balanceAfter = await this.ethProvider.getBalance(to);
    expect(balanceBefore.eq(balanceAfter), "The withdrawal did not fail the first time").to.be.true;

    // Make sure that the withdrawal will pass now
    const tx = await revertReceiveContract.setRevertReceive(false);
    await tx.wait();
    
    // Re-try
    const withdrawPendingTx = await withdrawalHelpers.withdrawPendingBalance(
        this.syncProvider,
        from.ethSigner.connect(this.ethProvider),
        to,
        'ETH'
    );
    await withdrawPendingTx.wait();

    // The funds should have arrived
    const expectedToBalance = balanceBefore.add(amount);
    const toBalance = await this.ethProvider.getBalance(to);    
    expect(toBalance.eq(expectedToBalance), "The withdrawal was not recovered").to.be.true;
}

Tester.prototype.testRecoverERC20Withdrawal = async function (
    from: Wallet,
    to: Address,
    token: TokenLike,
    amount: BigNumber
) {
    const tokenAddress = this.syncProvider.tokenSet.resolveTokenAddress(token);
    const revertTransferERC20 = RevertTransferERC20Factory.connect(
        tokenAddress,
        from.ethSigner
    );

    // Making sure that the withdrawal will be reverted 
    await revertTransferERC20.setRevertTransfer(true);

    const balanceBefore = await revertTransferERC20.balanceOf(to);
    const withdrawTx = await from.withdrawFromSyncToEthereum({
        ethAddress: to,
        token: token,
        amount
    });
    await withdrawTx.awaitVerifyReceipt();
    
    // Waiting for the withdrawl to be sent onchain
    const withdrawalTxHash = await waitForOnchainWithdrawal(
        this.syncProvider,
        withdrawTx.txHash
    );
    
    // Double-check that zkSync tried to process withdrawal
    expect(withdrawalTxHash, 'Withdrawal was not processed onchain').to.exist;

    // Double-check that the withdrawal has indeed failed
    const balanceAfter = await revertTransferERC20.balanceOf(to);
    expect(balanceBefore.eq(balanceAfter), "The withdrawal did not fail the first time").to.be.true;

    // Make sure that the withdrawal will pass now
    const tx = await revertTransferERC20.setRevertTransfer(false);
    await tx.wait();
    
    // Re-try
    const withdrawPendingTx = await withdrawalHelpers.withdrawPendingBalance(
        this.syncProvider,
        from.ethSigner.connect(this.ethProvider),
        to,
        token,
    );
    await withdrawPendingTx.wait();

    // The funds should have arrived
    const expectedToBalance = balanceBefore.add(amount);
    const toBalance = await revertTransferERC20.balanceOf(to);    
    expect(toBalance.eq(expectedToBalance), "The withdrawal was not recovered").to.be.true;
}
