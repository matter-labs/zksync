import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';

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

    const tokenAddress = initiatorWallet.provider.tokenSet.resolveTokenAddress(token);

    const onchainBalanceBefore = await targetWallet.getEthereumBalance(token);
    const balanceToWithdraw = await targetWallet.getBalance(token);

    const handle = await this.testForcedExit(initiatorWallet, targetWallet, token);

    // Await for verification with a timeout set (through mocha's --timeout)
    await handle.awaitVerifyReceipt();

    // Checking that there are some complete withdrawals tx hash for this ForcedExit
    // we should wait some time for `completeWithdrawals` transaction to be processed
    const withdrawalTxHash = await this.syncProvider.getEthTxForWithdrawal(handle.txHash);
    expect(withdrawalTxHash, 'Withdrawal was not processed onchain').to.exist;

    await this.ethProvider.waitForTransaction(withdrawalTxHash as string);

    const onchainBalanceAfter = await targetWallet.getEthereumBalance(token);
    const pendingToBeOnchain = await this.contract.getPendingBalance(targetWallet.address(), tokenAddress);

    expect(
        onchainBalanceAfter.add(pendingToBeOnchain).sub(onchainBalanceBefore).eq(balanceToWithdraw),
        'Wrong amount onchain after ForcedExit'
    ).to.be.true;
};

Tester.prototype.testForcedExit = async function (initiatorWallet: Wallet, targetWallet: Wallet, token: TokenLike) {
    // Forced exit is defined by `Withdraw` transaction type (as it's essentially just a forced withdraw).
    const { totalFee: fee } = await this.syncProvider.getTransactionFee('Withdraw', targetWallet.address(), token);
    const initiatorBalanceBefore = await initiatorWallet.getBalance(token);
    const handle = await initiatorWallet.syncForcedExit({
        target: targetWallet.address(),
        token,
        fee
    });

    const receipt = await handle.awaitReceipt();
    expect(receipt.success, `Withdraw transaction failed with a reason: ${receipt.failReason}`).to.be.true;

    const targetBalance = await targetWallet.getBalance(token);
    const initiatorBalanceAfter = await initiatorWallet.getBalance(token);
    expect(targetBalance.isZero(), 'Wrong amount in target wallet after ForcedExit').to.be.true;
    expect(
        initiatorBalanceBefore.sub(initiatorBalanceAfter).eq(fee),
        'Wrong amount in initiator wallet after ForcedExit'
    ).to.be.true;
    this.runningFee = this.runningFee.add(fee);
    return handle;
};
