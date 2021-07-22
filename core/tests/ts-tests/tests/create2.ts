import { expect } from 'chai';
import { Tester, expectThrow } from './tester';
import { Wallet, types, wallet } from 'zksync';
import { BigNumber, utils } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testCreate2CPKandTransfer(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testCreate2TxFail(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testCreate2BatchFail(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testCreate2SignedBatchFail(
            sender: Wallet,
            receiver: Wallet,
            token: TokenLike,
            amount: BigNumber
        ): Promise<void>;
    }
}

Tester.prototype.testCreate2CPKandTransfer = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    expect(await sender.isSigningKeySet(), "Pubkey shouldn't be set for this test").to.be.false;

    const senderBalance = await sender.getBalance(token);
    const receiverBalance = await receiver.getBalance(token);

    const nonce = await sender.getNonce();
    const cpkFee = await sender.provider.getTransactionFee({ ChangePubKey: 'CREATE2' }, sender.address(), token);
    const transferFee = await sender.provider.getTransactionFee('Transfer', receiver.address(), token);

    const changePubKey = await sender.signSetSigningKey({
        feeToken: token,
        fee: cpkFee.totalFee,
        nonce,
        ethAuthType: 'CREATE2'
    });

    const transfer = await sender.signSyncTransfer({
        to: receiver.address(),
        token,
        amount,
        fee: transferFee.totalFee,
        nonce: nonce + 1
    });

    const batch = [changePubKey, transfer];
    const txs = await wallet.submitSignedTransactionsBatch(sender.provider, batch, []);
    await Promise.all(txs.map((tx) => tx.awaitReceipt()));

    const fees = cpkFee.totalFee.add(transferFee.totalFee);
    const senderBalanceAfter = await sender.getBalance(token);
    const receiverBalanceAfter = await receiver.getBalance(token);
    expect(await sender.isSigningKeySet(), 'Pubkey should be set').to.be.true;
    expect(receiverBalanceAfter.sub(receiverBalance).eq(amount), 'Batch failed').to.be.true;
    expect(senderBalance.sub(senderBalanceAfter).eq(fees.add(amount)), 'Batch failed').to.be.true;

    this.runningFee = this.runningFee.add(fees);
};

Tester.prototype.testCreate2TxFail = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const fee = await sender.provider.getTransactionFee('Transfer', receiver.address(), token);
    const txData = await sender.signSyncTransfer({
        to: receiver.address(),
        token,
        amount,
        fee: fee.totalFee,
        nonce: await sender.getNonce()
    });
    txData.ethereumSignature = {
        type: 'EthereumSignature',
        // does not matter what bytes we pass as a signature
        signature: utils.hexlify(new Uint8Array(65))
    };
    await expectThrow(
        wallet.submitSignedTransaction(txData, sender.provider),
        'Eth signature from CREATE2 account not expected'
    );
};

Tester.prototype.testCreate2SignedBatchFail = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const batch = await sender.batchBuilder().addTransfer({ to: receiver.address(), token, amount }).build(token);

    batch.signature = {
        type: 'EthereumSignature',
        signature: utils.hexlify(new Uint8Array(65))
    };

    await expectThrow(
        wallet.submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]),
        'Eth signature from CREATE2 account not expected'
    );
};

Tester.prototype.testCreate2BatchFail = async function (
    sender: Wallet,
    receiver: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const batch: types.SignedTransaction[] = [];
    const fee = await sender.provider.getTransactionFee('Transfer', receiver.address(), token);
    const nonce = await sender.getNonce();

    for (let i = 0; i < 2; i++) {
        const txData = await sender.signSyncTransfer({
            to: receiver.address(),
            token,
            amount,
            fee: fee.totalFee,
            nonce: nonce + i
        });
        txData.ethereumSignature = {
            type: 'EthereumSignature',
            signature: utils.hexlify(new Uint8Array(65))
        };
        batch.push(txData);
    }

    await expectThrow(
        wallet.submitSignedTransactionsBatch(sender.provider, batch, []),
        'Eth signature from CREATE2 account not expected'
    );
};
