import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber, utils } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testTransfer(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testMultiTransfer(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testFailedMultiTransfer(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testWrongSignature(from: Wallet, to: Wallet): Promise<void>;
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

    // await sleep(timeoutBeforeReceipt);
    await handle.awaitReceipt();
    const senderAfter = await sender.getBalance(token);
    const receiverAfter = await receiver.getBalance(token);

    if (sender.address() === receiver.address()) {
        expect(senderBefore.sub(fee).eq(senderAfter), 'Transfer to self checks failed').to.be.true;
    } else {
        expect(senderBefore.sub(senderAfter).eq(amount.add(fee)), 'Transfer checks failed').to.be.true;
        expect(receiverAfter.sub(receiverBefore).eq(amount), 'Transfer checks failed').to.be.true;
    }

    this.runningFee = this.runningFee.add(fee);
};

Tester.prototype.testMultiTransfer = async function (
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
        fee: fee.div(2)
    };

    // First, execute batched transfers successfully.
    {
        const senderBefore = await sender.getBalance(token);
        const receiverBefore = await receiver.getBalance(token);
        const handles = await sender.syncMultiTransfer([{ ...tx }, { ...tx }]);
        await Promise.all(handles.map((handle) => handle.awaitReceipt()));
        const senderAfter = await sender.getBalance(token);
        const receiverAfter = await receiver.getBalance(token);
        expect(senderBefore.sub(senderAfter).eq(amount.mul(2).add(fee)), 'Batched transfer checks failed').to.be.true;
        expect(receiverAfter.sub(receiverBefore).eq(amount.mul(2)), 'Batched transfer checks failed').to.be.true;
    }

    // Then, send another batch in which the second transaction will fail.
    // The first transaction should not be executed.
    {
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
        expect(senderBefore.eq(senderAfter), 'Batched transfer checks failed').to.be.true;
        expect(receiverAfter.eq(receiverBefore), 'Batched transfer checks failed').to.be.true;
    }

    this.runningFee = this.runningFee.add(fee);
};

Tester.prototype.testFailedMultiTransfer = async function (
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
    try {
        const handles = await sender.syncMultiTransfer([{ ...tx }, { ...tx }]);
        for (const handle of handles) {
            await handle.awaitVerifyReceipt();
        }
        // this line should be unreachable
        expect.fail('This batch should have failed!');
    } catch (e) {
        expect(e.jrpcError.message).to.equal('Transactions batch summary fee is too low');
    }
};

Tester.prototype.testWrongSignature = async function (from: Wallet, to: Wallet) {
    const signedTransfer = await from.signSyncTransfer({
        to: to.address(),
        token: 'ETH',
        amount: utils.parseEther('0.002'),
        fee: utils.parseEther('0.001'),
        nonce: await from.getNonce()
    });

    const ETH_SIGNATURE_LENGTH_PREFIXED = 132;
    const fakeEthSignature: types.TxEthSignature = {
        signature: '0x'.padEnd(ETH_SIGNATURE_LENGTH_PREFIXED, '0'),
        type: 'EthereumSignature'
    };

    try {
        await from.provider.submitTx(signedTransfer.tx, fakeEthSignature);
        expect.fail('Sending tx with incorrect ETH signature must throw');
    } catch (e) {
        expect(e.jrpcError.message).to.equal('Eth signature is incorrect');
    }

    const { totalFee } = await this.syncProvider.getTransactionFee('Withdraw', from.address(), 'ETH');

    const signedWithdraw = await from.signWithdrawFromSyncToEthereum({
        ethAddress: from.address(),
        token: 'ETH',
        amount: utils.parseEther('0.001'),
        fee: totalFee,
        nonce: await from.getNonce()
    });

    try {
        await from.provider.submitTx(signedWithdraw.tx, fakeEthSignature);
        expect.fail('Sending tx with incorrect ETH signature must throw');
    } catch (e) {
        expect(e.jrpcError.message).to.equal('Eth signature is incorrect');
    }
};
