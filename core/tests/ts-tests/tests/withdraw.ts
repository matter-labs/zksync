import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, utils } from 'zksync';
import { BigNumber } from 'ethers';
import { sleep } from 'zksync/build/utils';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testVerifiedWithdraw(wallet: Wallet, token: TokenLike, amount: BigNumber, fast?: boolean): Promise<void>;
        testWithdraw(wallet: Wallet, token: TokenLike, amount: BigNumber, fast?: boolean): Promise<any>;
    }
}

Tester.prototype.testVerifiedWithdraw = async function (
    wallet: Wallet,
    token: TokenLike,
    amount: BigNumber,
    fastProcessing?: boolean
) {
    const tokenId = wallet.provider.tokenSet.resolveTokenId(token);

    const onchainBalanceBefore = await wallet.getEthereumBalance(token);
    const pendingBalanceBefore = await this.contract.getBalanceToWithdraw(wallet.address(), tokenId);
    const handle = await this.testWithdraw(wallet, token, amount, fastProcessing);

    // Await for verification with a timeout set (through mocha's --timeout)
    await handle.awaitVerifyReceipt();

    // Checking that there are some complete withdrawals tx hash for this withdrawal
    // we should wait some time for `completeWithdrawals` transaction to be processed
    let withdrawalTxHash = null;
    const polling_interval = 200; // ms
    const polling_timeout = 35000; // ms
    const polling_iterations = polling_timeout / polling_interval;
    for (let i = 0; i < polling_iterations; i++) {
        withdrawalTxHash = await this.syncProvider.getEthTxForWithdrawal(handle.txHash);
        if (withdrawalTxHash != null) {
            break;
        }
        await sleep(polling_interval);
    }
    expect(withdrawalTxHash, 'Withdrawal was not processed onchain').to.exist;

    await this.ethProvider.waitForTransaction(withdrawalTxHash as string);

    const onchainBalanceAfter = await wallet.getEthereumBalance(token);
    const pendingBalanceAfter = await this.contract.getBalanceToWithdraw(wallet.address(), tokenId);
    expect(
        onchainBalanceAfter.sub(onchainBalanceBefore).add(pendingBalanceAfter).sub(pendingBalanceBefore).eq(amount),
        'Wrong amount onchain after withdraw'
    ).to.be.true;
};

Tester.prototype.testWithdraw = async function (
    wallet: Wallet,
    token: TokenLike,
    amount: BigNumber,
    fastProcessing?: boolean
) {
    const type = fastProcessing ? 'FastWithdraw' : 'Withdraw';
    const { totalFee: fee } = await this.syncProvider.getTransactionFee(type, wallet.address(), token);
    const balanceBefore = await wallet.getBalance(token);

    const handle = await wallet.withdrawFromSyncToEthereum({
        ethAddress: wallet.address(),
        token,
        amount,
        fee,
        fastProcessing
    });

    const receipt = await handle.awaitReceipt();
    expect(receipt.success, `Withdraw transaction failed with a reason: ${receipt.failReason}`).to.be.true;

    const balanceAfter = await wallet.getBalance(token);
    expect(balanceBefore.sub(balanceAfter).eq(amount.add(fee)), 'Wrong amount in wallet after withdraw').to.be.true;
    this.runningFee = this.runningFee.add(fee);
    return handle;
};
