import { expect } from 'chai';
import { Tester, expectThrow } from './tester';
import { Wallet, types, wallet, Signer, No2FAWalletSigner } from 'zksync';
import { BigNumber, utils } from 'ethers';
import { Address } from '../../../../sdk/zksync.js/build/types';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testNo2FATransfer(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testNo2FASwap(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
    }
}

Tester.prototype.testNo2FATransfer = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    expect(await sender.isSigningKeySet(), 'Pubkey should be set for this test').to.be.true;

    const senderBalance = await sender.getBalance(token);
    const receiverBalance = await receiver.getBalance(token);

    const transferFee = await sender.provider.getTransactionFee('Transfer', receiver.address(), token);
    const fee = transferFee.totalFee;

    const transfer = await sender.syncTransfer({
        to: receiver.address(),
        token,
        amount,
        fee
    });

    await transfer.awaitReceipt();

    const senderBalanceAfter = await sender.getBalance(token);
    const receiverBalanceAfter = await receiver.getBalance(token);

    expect(receiverBalanceAfter.sub(receiverBalance).eq(amount), 'Transfer failed').to.be.true;
    expect(senderBalance.sub(senderBalanceAfter).eq(fee.add(amount)), 'Transfer failed').to.be.true;

    this.runningFee = this.runningFee.add(fee);
};

Tester.prototype.testNo2FASwap = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    // expect(await sender.isSigningKeySet(), "Pubkey shouldn't be set for this test").to.be.false;
    // const senderBalance = await sender.getBalance(token);
    // const receiverBalance = await receiver.getBalance(token);
    // const nonce = await sender.getNonce();
    // const cpkFee = await sender.provider.getTransactionFee({ ChangePubKey: 'CREATE2' }, sender.address(), token);
    // const transferFee = await sender.provider.getTransactionFee('Transfer', receiver.address(), token);
    // const changePubKey = await sender.signSetSigningKey({
    //     feeToken: token,
    //     fee: cpkFee.totalFee,
    //     nonce,
    //     ethAuthType: 'CREATE2'
    // });
    // const transfer = await sender.signSyncTransfer({
    //     to: receiver.address(),
    //     token,
    //     amount,
    //     fee: transferFee.totalFee,
    //     nonce: nonce + 1
    // });
    // const batch = [changePubKey, transfer];
    // const txs = await wallet.submitSignedTransactionsBatch(sender.provider, batch, []);
    // await Promise.all(txs.map((tx) => tx.awaitReceipt()));
    // const fees = cpkFee.totalFee.add(transferFee.totalFee);
    // const senderBalanceAfter = await sender.getBalance(token);
    // const receiverBalanceAfter = await receiver.getBalance(token);
    // expect(await sender.isSigningKeySet(), 'Pubkey should be set').to.be.true;
    // expect(receiverBalanceAfter.sub(receiverBalance).eq(amount), 'Batch failed').to.be.true;
    // expect(senderBalance.sub(senderBalanceAfter).eq(fees.add(amount)), 'Batch failed').to.be.true;
    // this.runningFee = this.runningFee.add(fees);
};
