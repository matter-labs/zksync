import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, utils } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;
const VERIFY_TIMEOUT = 120_000;

declare module './tester' {
    interface Tester {
        testWithdraw(from: Wallet, to: Wallet, token: TokenLike, amount: BigNumber, fast?: boolean): Promise<BigNumber>;
    }
}

function timeout<T>(promise: Promise<T>, millis: number) {
    const timeout = new Promise((_, reject) => {
        setTimeout(() => reject(`Timed out in ${millis}ms.`), millis);
    });
    return Promise.race([promise, timeout]);
}

Tester.prototype.testWithdraw = async function (
    from: Wallet,
    to: Wallet,
    token: TokenLike,
    amount: BigNumber,
    fastProcessing?: boolean
) {
    const { totalFee: fee } = await this.syncProvider.getTransactionFee('Withdraw', to.address(), token);
    const balanceBefore = await from.getBalance(token);
    const onchainBalanceBefore = await to.getEthereumBalance(token);

    const handle = await from.withdrawFromSyncToEthereum({
        ethAddress: to.address(),
        token,
        amount,
        fee,
        fastProcessing
    });

    // Checking that there are no complete withdrawals tx hash for this withdrawal
    expect(await this.syncProvider.getEthTxForWithdrawal(handle.txHash)).to.be.empty;

    // Await for verification with a timeout set.
    await timeout(handle.awaitVerifyReceipt(), VERIFY_TIMEOUT);

    // Checking that there are some complete withdrawals tx hash for this withdrawal
    // we should wait some time for `completeWithdrawals` transaction to be processed
    await utils.sleep(10000);
    expect(await this.syncProvider.getEthTxForWithdrawal(handle.txHash)).to.not.be.empty;

    const balanceAfter = await from.getBalance(token);
    const onchainBalanceAfter = await to.getEthereumBalance(token);
    const tokenId = to.provider.tokenSet.resolveTokenId(token);
    const pendingToBeOnchain = await this.contract.getBalanceToWithdraw(to.address(), tokenId);
    expect(balanceBefore.sub(balanceAfter).eq(amount.add(fee)), 'Wrong amount on wallet after withdraw').to.be.true;
    expect(
        onchainBalanceAfter.add(pendingToBeOnchain).sub(onchainBalanceBefore).eq(amount),
        'Wrong amount onchain after withdraw'
    ).to.be.true;

    return fee;
};
