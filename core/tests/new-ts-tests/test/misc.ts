import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { utils } from 'ethers';

declare module './tester' {
    interface Tester {
        testWrongSignature(from: Wallet, to: Wallet): Promise<void>;
        testTransactionResending(from: Wallet, to: Wallet): Promise<void>;
    }
}

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

// Tester.prototype.testTransactionResending = async function (
//     from: Wallet,
//     to: Wallet
// ) {
//     const amount = utils.parseEther("0.2");
//     const transferFullFee = await this.syncProvider.getTransactionFee("Transfer", to.address(), "ETH");
//     const transferFee = transferFullFee.totalFee;
//
//     const feeType = { ChangePubKey: { onchainPubkeyAuth: false } };
//     const changePubKeyFullFee = await this.syncProvider.getTransactionFee(feeType, from.address(), "ETH");
//     const changePubKeyFee = changePubKeyFullFee.totalFee;
//
//     await this.testDeposit(
//         from,
//         "ETH",
//         amount.div(2).add(transferFee).add(changePubKeyFee),
//         true
//     );
//     await this.testChangePubKey(from, "ETH", true);
//     try {
//         await this.testTransfer(from, to, "ETH", amount);
//         expect.fail();
//     } catch (e) {
//         console.log(JSON.stringify(e, null, 4));
//         expect(e.value.failReason).to.equal('Not enough balance');
//     }
//
//     await this.testDeposit(from, "ETH", amount.div(2));
//     // We should wait some `timeoutBeforeReceipt` to give server enough time
//     // to move our transaction with success flag from mempool to statekeeper
//     //
//     // If we won't wait enough, then we'll get the receipt for the previous, failed tx,
//     // which has the same hash. The new (successful) receipt will be available only
//     // when tx will be executed again in state keeper, so we must wait for it.
//     // await this.testTransfer(from, to, "ETH", amount, 3000);
// }
