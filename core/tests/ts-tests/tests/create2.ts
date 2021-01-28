import { Tester, expectThrow } from './tester';
import { Wallet, types, wallet } from 'zksync';
import { BigNumber, utils } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testCreate2TxFail(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testCreate2BatchFail(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
        testCreate2SignedBatchFail(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber): Promise<void>;
    }
}

Tester.prototype.testCreate2TxFail = async function(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber) {
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
}

Tester.prototype.testCreate2SignedBatchFail = async function(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber) {
    const batch = await sender
        .batchBuilder()
        .addTransfer({ to: receiver.address(), token, amount })
        .build(token)

    batch.signature = {
        type: 'EthereumSignature',
        signature: utils.hexlify(new Uint8Array(65))
    };

    await expectThrow(
        wallet.submitSignedTransactionsBatch(sender.provider, batch.txs, [batch.signature]),
        'Eth signature from CREATE2 account not expected'
    );
}

Tester.prototype.testCreate2BatchFail = async function(sender: Wallet, receiver: Wallet, token: TokenLike, amount: BigNumber) {
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
}
