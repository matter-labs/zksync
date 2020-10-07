import { expect, use } from 'chai';
import promised from 'chai-as-promised';
import { BigNumber, utils } from 'ethers';
import { Wallet } from 'zksync';

import { Tester } from './tester';
import './deposit';
import './change-pub-key';
import './transfer';
import './withdraw';

use(promised);

describe('ZkSync integration tests', () => {
    let tester: Tester;
    const one = utils.parseEther('1.0');
    // const hundred = utils.parseEther('100.0');
    let alice: Wallet;
    // let bob: Wallet;
    let carl: Wallet;
    // let donna: Wallet;
    let operatorBalance: BigNumber;

    before('create tester', async () => {
        tester = await Tester.init('localhost', 'HTTP');
        alice = await tester.fundedWallet('1.45');
        // bob = await tester.emptyWallet();
        carl = await tester.emptyWallet();
        // donna = await tester.emptyWallet();
        operatorBalance = await tester.operatorBalance('ETH');
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    step('should execute an auto-approved deposit', async () => {
        await expect(tester.testDeposit(alice, 'ETH', one, true)).to.be.fulfilled;
        // await expect(tester.testDeposit(bob, 'DAI', hundred, true)).to.be.fulfilled;
    });

    step('should execute a normal deposit', async () => {
        await expect(tester.testDeposit(alice, 'ETH', one)).to.be.fulfilled;

        // expect(await tester.syncWallet.isERC20DepositsApproved('DAI'), "Token should not be approved").to.be.false;
        // const approveERC20 = await tester.syncWallet.approveERC20TokenDeposits('DAI');
        // await approveERC20.wait();
        // expect(await tester.syncWallet.isERC20DepositsApproved('DAI'), "Token should be approved").to.be.true;
        // await expect(tester.testDeposit(bob, 'DAI', hundred)).to.be.fulfilled;
        // expect(await tester.syncWallet.isERC20DepositsApproved('DAI'), "Token should still be approved").to.be.true;
    });

    step('should change pubkey onchain', async () => {
        await expect(tester.testChangePubKey(alice, 'ETH', true)).to.be.fulfilled;
        // await expect(tester.testChangePubKey(bob, 'DAI', true)).to.be.fulfilled;
    });

    step('should execute a transfer to new account', async () => {
        await expect(tester.testTransfer(alice, carl, 'ETH', one.div(10))).to.be.fulfilled;
        // await expect(tester.testTransfer(bob, donna, 'DAI', hundred.div(10))).to.be.fulfilled;
    });

    step('should execute a transfer to existing account', async () => {
        await expect(tester.testTransfer(alice, carl, 'ETH', one.div(10))).to.be.fulfilled;
        // await expect(tester.testTransfer(bob, donna, 'DAI', hundred.div(10))).to.be.fulfilled;
    });

    step('should execute a transfer to self', async () => {
        await expect(tester.testTransfer(alice, alice, 'ETH', one.div(10))).to.be.fulfilled;
        // await expect(tester.testTransfer(bob, bob, 'DAI', hundred.div(10))).to.be.fulfilled;
    });

    step('should change pubkey offchain', async () => {
        await expect(tester.testChangePubKey(alice, 'ETH', false)).to.be.fulfilled;
        await expect(tester.testChangePubKey(carl, 'ETH', false)).to.be.fulfilled;
        // await expect(tester.testChangePubKey(bob, 'DAI', false)).to.be.fulfilled;
    });

    step('should test multi-transfers', async () => {
        await expect(tester.testMultiTransfer(alice, carl, 'ETH', one.div(100))).to.be.fulfilled;
        await expect(tester.testFailedMultiTransfer(alice, carl, 'ETH', one.div(100))).to.be.fulfilled;
    });

    step('should execute a withdrawal', async () => {
        await expect(tester.testWithdraw(alice, 'ETH', one.div(10))).to.be.fulfilled;
    });

    step('should execute a fast withdrawal', async () => {
        await expect(tester.testWithdraw(carl, 'ETH', one.div(10), true)).to.be.fulfilled;
    });

    step('check collected fees', async () => {
        const collectedFee = (await tester.operatorBalance('ETH')).sub(operatorBalance);
        expect(collectedFee.eq(tester.runningFee), 'Fee collection failed').to.be.true;
    });
});
