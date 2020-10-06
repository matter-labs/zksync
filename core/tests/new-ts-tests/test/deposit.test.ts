import { Tester } from './tester';
import { expect, use } from 'chai';
import promised from 'chai-as-promised';
import { Wallet, types, utils } from 'zksync';
import { BigNumberish } from 'ethers';

use(promised);

declare module './tester' {
    interface Tester {
        testAutoApprovedDeposit(
            wallet: Wallet,
            token: types.TokenLike,
            amount: BigNumberish
        ): Promise<void>;
        testDeposit(
            wallet: Wallet,
            token: types.TokenLike,
            amount: BigNumberish
        ): Promise<void>;
    }
}

Tester.prototype.testAutoApprovedDeposit = async function(
    wallet: Wallet,
    token: types.TokenLike,
    _amount: string
) {
    const amount = this.syncProvider.tokenSet.parseToken(token, _amount);
    const balanceBefore = await wallet.getBalance(token);

    const depositHandle = await this.richWallet.depositToSyncFromEthereum({
        depositTo: wallet.address(),
        token: token,
        amount,
        approveDepositAmountForERC20: true,
    });

    await depositHandle.awaitReceipt();
    const balanceAfter = await wallet.getBalance(token);
    expect(balanceAfter.sub(balanceBefore).eq(amount), "Deposit checks failed").to.be.true;
}

Tester.prototype.testDeposit = async function(wallet: Wallet, token: types.TokenLike, _amount: string) {
    const amount = this.syncProvider.tokenSet.parseToken(token, _amount);
    const balanceBefore = await wallet.getBalance(token);

    if (!utils.isTokenETH(token)) {
        expect(await this.richWallet.isERC20DepositsApproved(token), "Token should not be approved").to.be.false;
        const approveERC20 = await this.richWallet.approveERC20TokenDeposits(token);
        await approveERC20.wait();
        expect(await this.richWallet.isERC20DepositsApproved(token), "Token should be approved").to.be.true;
    }

    const depositHandle = await this.richWallet.depositToSyncFromEthereum({
        depositTo: wallet.address(),
        token: token,
        amount,
    });

    await depositHandle.awaitReceipt();
    const balanceAfter = await wallet.getBalance(token);
    if (!utils.isTokenETH(token)) {
        expect(await this.richWallet.isERC20DepositsApproved(token), "Token should still be approved").to.be.true;
    }
    expect(balanceAfter.sub(balanceBefore).eq(amount), "Deposit checks failed").to.be.true;
}


describe('Test zkSync deposits', () => {
    let tester: Tester;

    before('create tester', async () => {
        tester = await Tester.init('localhost', 'HTTP');
    })

    it('should execute auto-approved deposit', async () => {
        let wallet = await tester.emptyWallet();
        await expect(tester.testAutoApprovedDeposit(wallet, 'DAI', '20.0')).to.be.fulfilled;
        await expect(tester.testAutoApprovedDeposit(wallet, 'ETH', '1.0')).to.be.fulfilled;
    });

    it('should execute a normal deposit', async () => {
        let wallet = await tester.emptyWallet();
        await expect(tester.testDeposit(wallet, 'DAI', '20.0')).to.be.fulfilled;
        await expect(tester.testDeposit(wallet, 'ETH', '1.0')).to.be.fulfilled;
    });
});

