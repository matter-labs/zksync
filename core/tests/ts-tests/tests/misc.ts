import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, utils } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testWrongSignature(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testTransactionResending(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
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

Tester.prototype.testTransactionResending = async function (
    from: Wallet,
    to: Wallet,
    token: TokenLike,
    amount: BigNumber
) {
    const transferFullFee = await this.syncProvider.getTransactionFee('Transfer', to.address(), token);
    const transferFee = transferFullFee.totalFee;

    const feeType = { ChangePubKey: { onchainPubkeyAuth: false } };
    const changePubKeyFullFee = await this.syncProvider.getTransactionFee(feeType, from.address(), token);
    const changePubKeyFee = changePubKeyFullFee.totalFee;

    const insufficientDepositAmount = amount.div(2).add(transferFee).add(changePubKeyFee);
    await this.testDeposit(from, token, insufficientDepositAmount, true);
    await this.testChangePubKey(from, token, true);

    let thrown = true;
    try {
        await this.testTransfer(from, to, token, amount);
        thrown = false; // this line should be unreachable
    } catch (e) {
        expect(e.value.failReason).to.equal('Not enough balance');
    }
    expect(thrown).to.be.true;

    // We provide more funds here, so that test won't randomly fail if the expected fee has changed in server.
    const sufficientDepositAmount = amount.add(transferFee).add(changePubKeyFee);
    await this.testDeposit(from, token, sufficientDepositAmount, true);

    // We should wait some `timeoutBeforeReceipt` to give server enough time
    // to move our transaction with success flag from mempool to statekeeper
    //
    // If we won't wait enough, then we'll get the receipt for the previous, failed tx,
    // which has the same hash. The new (successful) receipt will be available only
    // when tx will be executed again in state keeper, so we must wait for it.
    await this.testTransfer(from, to, token, amount, 3000);
};
