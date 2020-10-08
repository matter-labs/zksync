import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testChangePubKey(wallet: Wallet, feeToken: TokenLike, onchain: boolean): Promise<void>;
    }
}

Tester.prototype.testChangePubKey = async function (wallet: Wallet, feeToken: TokenLike, onchain: boolean) {
    if (await wallet.isSigningKeySet()) return;

    const feeType = { ChangePubKey: { onchainPubkeyAuth: onchain } };
    let { totalFee: fee } = await this.syncProvider.getTransactionFee(feeType, wallet.address(), feeToken);

    if (onchain) {
        const handle = await wallet.onchainAuthSigningKey();
        await handle.wait();
    }

    const changePubkeyHandle = await wallet.setSigningKey({
        feeToken,
        fee,
        onchainAuth: onchain
    });

    await changePubkeyHandle.awaitReceipt();
    expect(await wallet.isSigningKeySet(), 'ChangePubKey failed').to.be.true;
    this.runningFee = this.runningFee.add(fee);
};
