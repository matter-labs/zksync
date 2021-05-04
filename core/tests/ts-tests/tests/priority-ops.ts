import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types } from 'zksync';
import { BigNumber } from 'ethers';

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testDeposit(wallet: Wallet, token: TokenLike, amount: BigNumber, approve?: boolean): Promise<void>;
        testFullExit(wallet: Wallet, token: TokenLike, accountId?: number): Promise<[BigNumber, BigNumber]>;
        testFullExitNFT(wallet: Wallet, accountId?: number): Promise<void>;
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

    const receipt = await depositHandle.awaitReceipt();
    expect(receipt.executed, 'Deposit was not executed').to.be.true;
    const balanceAfter = await wallet.getBalance(token);
    expect(
        balanceAfter.sub(balanceBefore).eq(amount),
        `Deposit balance mismatch. Expected ${amount}, actual ${balanceAfter.sub(balanceBefore)}`
    ).to.be.true;
};

Tester.prototype.testFullExit = async function (wallet: Wallet, token: TokenLike, accountId?: number) {
    const balanceBefore = await wallet.getBalance(token);
    const handle = await wallet.emergencyWithdraw({ token, accountId });
    let receipt = await handle.awaitReceipt();
    expect(receipt.executed, 'Full Exit was not executed').to.be.true;
    const balanceAfter = await wallet.getBalance(token);
    return [balanceBefore, balanceAfter];
};

Tester.prototype.testFullExitNFT = async function (wallet: Wallet, accountId?: number) {
    const state = await wallet.getAccountState();
    let nft: any = Object.values(state.committed.nfts)[0];

    const balanceBefore = await wallet.getNFT(nft.id);
    const handle = await wallet.emergencyWithdrawNFT({ tokenId: nft.id, accountId });
    let receipt = await handle.awaitReceipt();
    expect(receipt.executed, 'NFT Full Exit was not executed').to.be.true;
    const balanceAfter = await wallet.getNFT(nft.id);

    expect(balanceBefore.id == nft.id, 'NFT Full Exit failed').to.be.true;
    expect(balanceAfter === undefined, 'NFT Full Exit failed').to.be.true;
};
