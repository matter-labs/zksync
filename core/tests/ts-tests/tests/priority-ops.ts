import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testDeposit(wallet: Wallet, token: TokenLike, amount: BigNumber, approve?: boolean): Promise<void>;
        testFullExit(wallet: Wallet, token: TokenLike, accountId?: number): Promise<[BigNumber, BigNumber]>;
    }
}

Tester.prototype.testDeposit = async function (wallet: Wallet, token: TokenLike, amount: BigNumber, approve?: boolean) {
    const balanceBefore = await wallet.getBalance(token);

    const depositHandle = await this.syncWallet.depositToSyncFromEthereum({
        depositTo: wallet.address(),
        token: token,
        amount,
        approveDepositAmountForERC20: approve
    });

    await depositHandle.awaitReceipt();
    const balanceAfter = await wallet.getBalance(token);
    expect(balanceAfter.sub(balanceBefore).eq(amount), 'Deposit failed').to.be.true;
};

Tester.prototype.testFullExit = async function (wallet: Wallet, token: TokenLike, accountId?: number) {
    const balanceBefore = await wallet.getBalance(token);
    const handle = await wallet.emergencyWithdraw({ token, accountId });
    await handle.awaitReceipt();
    const balanceAfter = await wallet.getBalance(token);
    return [balanceBefore, balanceAfter];
};
