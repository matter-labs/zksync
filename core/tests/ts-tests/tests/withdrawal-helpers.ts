import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, Provider, utils } from 'zksync';
import { BigNumber, ethers } from 'ethers';
import { Address } from 'zksync/build/types';
import { sleep } from 'zksync/build/utils';

import { RevertReceiveAccountFactory, RevertTransferERC20Factory } from '../../../../contracts/typechain';
import { loadTestConfig } from 'reading-tool';

const TEST_CONFIG = loadTestConfig(true);

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testRecoverETHWithdrawal(from: Wallet, to: Address, amount: BigNumber): Promise<void>;
        testRecoverERC20Withdrawal(from: Wallet, to: Address, token: TokenLike, amount: BigNumber): Promise<void>;
        testRecoverMultipleWithdrawals(
            from: Wallet,
            to: Address[],
            token: TokenLike[],
            amount: BigNumber[]
        ): Promise<void>;
    }
}

async function waitForOnchainWithdrawal(
    syncProvider: Provider,
    hash: string,
    polling_interval: number = 200,
    polling_timeout: number = 35000
): Promise<string | null> {
    let withdrawalTxHash = null;
    const polling_iterations = polling_timeout / polling_interval;
    for (let i = 0; i < polling_iterations; i++) {
        withdrawalTxHash = await syncProvider.getEthTxForWithdrawal(hash);
        if (withdrawalTxHash != null) {
            break;
        }
        await sleep(polling_interval);
    }

    return withdrawalTxHash;
}

async function setRevertReceive(ethWallet: ethers.Signer, to: Address, value: boolean) {
    const revertReceiveContract = RevertReceiveAccountFactory.connect(to, ethWallet);

    const tx = await revertReceiveContract.setRevertReceive(value);
    await tx.wait();
}

async function setRevertTransfer(ethWallet: ethers.Signer, tokenAddress: Address, value: boolean) {
    const revertTransferERC20 = RevertTransferERC20Factory.connect(tokenAddress, ethWallet);

    const tx = await revertTransferERC20.setRevertTransfer(value);
    await tx.wait();
}

async function setRevert(
    ethWallet: ethers.Signer,
    provider: Provider,
    recipient: Address,
    token: TokenLike,
    value: boolean
) {
    const tokenSymbol = provider.tokenSet.resolveTokenSymbol(token);
    const tokenAddress = provider.tokenSet.resolveTokenAddress(token);

    if (tokenSymbol === 'ETH') {
        await setRevertReceive(ethWallet, recipient, value);
    } else {
        await setRevertTransfer(ethWallet, tokenAddress, value);
    }
}

Tester.prototype.testRecoverETHWithdrawal = async function (from: Wallet, to: Address, amount: BigNumber) {
    // Make sure that the withdrawal will fail
    await setRevert(from.ethSigner, this.syncProvider, to, 'ETH', true);
    const balanceBefore = await this.ethProvider.getBalance(to);
    const withdrawTx = await from.withdrawFromSyncToEthereum({
        ethAddress: to,
        token: 'ETH',
        amount
    });
    await withdrawTx.awaitVerifyReceipt();
    // Wait for the withdrawl to be sent onchain
    const withdrawalTxHash = await waitForOnchainWithdrawal(this.syncProvider, withdrawTx.txHash);

    // Double-check that zkSync tried to process withdrawal
    expect(withdrawalTxHash, 'Withdrawal was not processed onchain').to.exist;

    // double-check that the withdrawal has indeed failed
    const balanceAfter = await this.ethProvider.getBalance(to);
    expect(balanceBefore.eq(balanceAfter), 'The withdrawal did not fail the first time').to.be.true;

    // Make sure that the withdrawal will pass now
    await setRevert(from.ethSigner, this.syncProvider, to, 'ETH', false);

    // Re-try
    const withdrawPendingTx = await from.withdrawPendingBalance(to, 'ETH');
    await withdrawPendingTx.wait();

    // The funds should have arrived
    const expectedToBalance = balanceBefore.add(amount);
    const toBalance = await this.ethProvider.getBalance(to);
    expect(toBalance.eq(expectedToBalance), 'The withdrawal was not recovered').to.be.true;
};

Tester.prototype.testRecoverERC20Withdrawal = async function (
    from: Wallet,
    to: Address,
    token: TokenLike,
    amount: BigNumber
) {
    // Make sure that the withdrawal will be reverted
    await setRevert(from.ethSigner, from.provider, to, token, true);

    const getToBalance = () =>
        utils.getEthereumBalance(from.ethSigner.provider as ethers.providers.Provider, from.provider, to, token);

    const balanceBefore = await getToBalance();
    const withdrawTx = await from.withdrawFromSyncToEthereum({
        ethAddress: to,
        token: token,
        amount
    });
    await withdrawTx.awaitVerifyReceipt();

    // Wait for the withdrawl to be sent onchain
    const withdrawalTxHash = await waitForOnchainWithdrawal(this.syncProvider, withdrawTx.txHash);

    // Double-check that zkSync tried to process withdrawal
    expect(withdrawalTxHash, 'Withdrawal was not processed onchain').to.exist;

    // Double-check that the withdrawal has indeed failed
    const balanceAfter = await getToBalance();
    expect(balanceBefore.eq(balanceAfter), 'The withdrawal did not fail the first time').to.be.true;

    // Make sure that the withdrawal will pass now
    await setRevert(from.ethSigner, from.provider, to, token, false);

    // Re-try
    const withdrawPendingTx = await from.withdrawPendingBalance(to, token);
    await withdrawPendingTx.wait();

    // The funds should have arrived
    const expectedToBalance = balanceBefore.add(amount);
    const toBalance = await getToBalance();
    expect(toBalance.eq(expectedToBalance), 'The withdrawal was not recovered').to.be.true;
};

Tester.prototype.testRecoverMultipleWithdrawals = async function (
    from: Wallet,
    to: Address[],
    token: TokenLike[],
    amount: BigNumber[]
) {
    const balancesBefore = await Promise.all(
        to.map(async (recipient, i) => {
            return utils.getEthereumBalance(this.ethProvider, this.syncProvider, recipient, token[i]);
        })
    );

    // Make sure that all the withdrawal will fall
    for (let i = 0; i < to.length; i++) {
        await setRevert(from.ethSigner, this.syncProvider, to[i], token[i], true);
    }

    // Send the withdrawals and wait until they are sent onchain
    for (let i = 0; i < to.length; i++) {
        const withdrawTx = await from.withdrawFromSyncToEthereum({
            ethAddress: to[i],
            token: token[i],
            amount: amount[i]
        });
        await withdrawTx.awaitVerifyReceipt();

        const withdrawalTxHash = await waitForOnchainWithdrawal(this.syncProvider, withdrawTx.txHash);
        expect(withdrawalTxHash, 'Withdrawal was not processed onchain').to.exist;
    }

    const balancesAfter = await Promise.all(
        to.map(async (recipient, i) => {
            return utils.getEthereumBalance(this.ethProvider, this.syncProvider, recipient, token[i]);
        })
    );

    // Check that all the withdrawals indeed failed
    balancesBefore.forEach((balance, i) => {
        expect(balance.eq(balancesAfter[i]), `The withdrawal ${i} did not fail the first time`).to.be.true;
    });

    // Make sure that all the withdrawal will pass now
    for (let i = 0; i < to.length; i++) {
        await setRevert(from.ethSigner, this.syncProvider, to[i], token[i], false);
    }

    const handle = await from.withdrawPendingBalances(to, token, {
        address: TEST_CONFIG.withdrawalHelpers.multicall_address
    });
    await handle.wait();

    const balancesAfterRecovery = await Promise.all(
        to.map(async (recipient, i) => {
            return utils.getEthereumBalance(this.ethProvider, this.syncProvider, recipient, token[i]);
        })
    );

    // The funds should have arrived
    balancesAfterRecovery.forEach((balance, i) => {
        expect(balance.eq(balancesBefore[i].add(amount[i])), `The withdrawal ${i} was not recovered`).to.be.true;
    });
};
