import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, utils } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testVerifiedForcedExit(initiatorWallet: Wallet, targetWallet: Wallet, token: TokenLike): Promise<void>;
        testForcedExit(initiatorWallet: Wallet, targetWallet: Wallet, token: TokenLike): Promise<any>;
    }
}

Tester.prototype.testVerifiedForcedExit = async function (
    initiatorWallet: Wallet,
    targetWallet: Wallet,
    token: TokenLike
) {
    // Forced exit is defined by `Withdraw` transaction type (as it's essentially just a forced withdraw),
    // therefore, when making requests to `syncProvider`, we will use the type `Withdraw`.

    const tokenId = initiatorWallet.provider.tokenSet.resolveTokenId(token);

    const onchainBalanceBefore = await targetWallet.getEthereumBalance(token);
    const initiatorBalanceBefore = await initiatorWallet.getBalance(token);
    const balanceToWithdraw = await targetWallet.getBalance(token);

    const handle = await this.testForcedExit(initiatorWallet, targetWallet, token);
    const { totalFee: fee } = await this.syncProvider.getTransactionFee('Withdraw', initiatorWallet.address(), token);

    // Await for verification with a timeout set (through mocha's --timeout)
    await handle.awaitVerifyReceipt();

    const onchainBalanceAfter = await targetWallet.getEthereumBalance(token);
    const pendingToBeOnchain = await this.contract.getBalanceToWithdraw(targetWallet.address(), tokenId);
    const initiatorBalanceAfter = await initiatorWallet.getBalance(token);

    // Wait some time for Ethereum transaction to be processed.
    await utils.sleep(10_000);

    expect(
        onchainBalanceAfter.add(pendingToBeOnchain).sub(onchainBalanceBefore).eq(balanceToWithdraw),
        'Wrong amount onchain after ForcedExit'
    ).to.be.true;
    expect(
        initiatorBalanceBefore.sub(initiatorBalanceAfter).eq(fee),
        'Wrong spent fee by Initiator Wallet after ForcedExit'
    ).to.be.true;
};

Tester.prototype.testForcedExit = async function (initiatorWallet: Wallet, targetWallet: Wallet, token: TokenLike) {
    // Forced exit is defined by `Withdraw` transaction type (as it's essentially just a forced withdraw).
    const { totalFee: fee } = await this.syncProvider.getTransactionFee('Withdraw', initiatorWallet.address(), token);
    const handle = await initiatorWallet.syncForcedExit({
        target: targetWallet.address(),
        token,
        fee
    });

    await handle.awaitReceipt();
    const balanceAfter = await targetWallet.getBalance(token);
    expect(balanceAfter.isZero(), 'Wrong amount in wallet after ForcedExit').to.be.true;
    this.runningFee = this.runningFee.add(fee);
    return handle;
};
