import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { ChangePubkeyTypes } from 'zksync/build/types';
import * as zksync from 'zksync';
import { utils } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testChangePubKey(wallet: Wallet, feeToken: TokenLike, onchain: boolean): Promise<void>;
    }
}

Tester.prototype.testChangePubKey = async function (wallet: Wallet, feeToken: TokenLike, onchain: boolean) {
    if (await wallet.isSigningKeySet()) return;

    const ethAuthType: ChangePubkeyTypes = onchain ? 'Onchain' : 'EIP712';

    const feeType = { ChangePubKey: ethAuthType };
    let { totalFee: fee } = await this.syncProvider.getTransactionFee(feeType, wallet.address(), feeToken);

    if (onchain) {
        const handle = await wallet.onchainAuthSigningKey();
        await handle.wait();
        expect(await wallet.isOnchainAuthSigningKeySet(), 'ChangePubKey is unset onchain').to.be.true;
    }

    const changePubkeyHandle = await wallet.setSigningKey({
        feeToken,
        fee,
        ethAuthType
    });

    const receipt = await changePubkeyHandle.awaitReceipt();
    expect(receipt.success, `ChangePubKey transaction failed with a reason: ${receipt.failReason}`).to.be.true;
    expect(await wallet.isSigningKeySet(), 'ChangePubKey failed').to.be.true;
    expect(await wallet.isCorrespondingSigningKeySet(), 'ChangePubKey failed').to.be.true;
    const oldSigner = wallet.signer;
    wallet.signer = await zksync.Signer.fromSeed(utils.randomBytes(32));
    expect(await wallet.isSigningKeySet(), 'ChangePubKey failed').to.be.true;
    expect(await wallet.isCorrespondingSigningKeySet(), 'Wrong signer for ChangePubKey failed').to.be.false;
    wallet.signer = oldSigner;
    const accountState = await wallet.getAccountState();
    expect(accountState.accountType, 'Incorrect account type').to.be.eql('Owned');

    this.runningFee = this.runningFee.add(fee);
};
