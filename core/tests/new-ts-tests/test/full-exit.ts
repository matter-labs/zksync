import { Tester } from './tester';
import { Wallet, types } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testFullExit(wallet: Wallet, token: TokenLike, accountId?: number): Promise<[BigNumber, BigNumber]>;
    }
}

Tester.prototype.testFullExit = async function (wallet: Wallet, token: TokenLike, accountId?: number) {
    const balanceBefore = await wallet.getBalance(token);
    const handle = await wallet.emergencyWithdraw({ token, accountId });
    await handle.awaitReceipt();
    const balanceAfter = await wallet.getBalance(token);
    return [balanceBefore, balanceAfter];
};
