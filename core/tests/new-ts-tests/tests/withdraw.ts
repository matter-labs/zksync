import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, utils } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;
const VERIFY_TIMEOUT = 120_000;

declare module './tester' {
    interface Tester {
        testVerifiedWithdraw(wallet: Wallet, token: TokenLike, amount: BigNumber, fast?: boolean): Promise<void>;
        testWithdraw(wallet: Wallet, token: TokenLike, amount: BigNumber, fast?: boolean): Promise<any>;
    }
}

function timeout<T>(promise: Promise<T>, millis: number) {
    const timeout = new Promise((_, reject) => {
        setTimeout(() => reject(`Timed out in ${millis}ms.`), millis);
    });
    return Promise.race([promise, timeout]);
}

Tester.prototype.testVerifiedWithdraw = async function (
    wallet: Wallet,
    token: TokenLike,
    amount: BigNumber,
    fastProcessing?: boolean
) {
    const onchainBalanceBefore = await wallet.getEthereumBalance(token);
    const handle = await this.testWithdraw(wallet, token, amount, fastProcessing);

    // Checking that there are no complete withdrawals tx hash for this withdrawal
    expect(await this.syncProvider.getEthTxForWithdrawal(handle.txHash)).to.not.exist;

    // Await for verification with a timeout set.
    await timeout(handle.awaitVerifyReceipt(), VERIFY_TIMEOUT);

    // Checking that there are some complete withdrawals tx hash for this withdrawal
    // we should wait some time for `completeWithdrawals` transaction to be processed
    await utils.sleep(10_000);
    expect(await this.syncProvider.getEthTxForWithdrawal(handle.txHash)).to.exist;

    const onchainBalanceAfter = await wallet.getEthereumBalance(token);
    const tokenId = wallet.provider.tokenSet.resolveTokenId(token);
    const pendingToBeOnchain = await this.contract.getBalanceToWithdraw(wallet.address(), tokenId);
    expect(
        onchainBalanceAfter.add(pendingToBeOnchain).sub(onchainBalanceBefore).eq(amount),
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

    await handle.awaitReceipt();
    const balanceAfter = await wallet.getBalance(token);
    expect(balanceBefore.sub(balanceAfter).eq(amount.add(fee)), 'Wrong amount on wallet after withdraw').to.be.true;
    this.runningFee = this.runningFee.add(fee);
    return handle;
};
