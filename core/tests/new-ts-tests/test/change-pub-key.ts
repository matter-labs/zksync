import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testChangePubKey(wallet: Wallet, feeToken: TokenLike, onchain: boolean): Promise<BigNumber>;
    }
}

Tester.prototype.testChangePubKey = async function (wallet: Wallet, feeToken: TokenLike, onchain: boolean) {
    if (await wallet.isSigningKeySet()) {
        return BigNumber.from(0);
    }

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
    return fee;
};
