import { expect, use } from 'chai';
import promised from 'chai-as-promised';
import { utils } from 'ethers';
import { Wallet } from 'zksync';

import { Tester } from './tester';
import './deposit';
import './change-pub-key';
import './transfer';

use(promised);

describe('ZkSync integration tests', () => {
    let tester: Tester;
    const one = utils.parseEther('1.0');
    // const hundred = utils.parseEther('100.0');
    let alice: Wallet;
    // let bob: Wallet;
    let carl: Wallet;
    // let donna: Wallet;

    before('create tester', async () => {
        tester = await Tester.init('localhost', 'HTTP');
        alice = await tester.fundedWallet('1.45');
        // bob = await tester.emptyWallet();
        carl = await tester.emptyWallet();
        // donna = await tester.emptyWallet();
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    describe('Deposit', () => {
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
    });

    describe('ChangePubKey and Transfer', () => {

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
            // await expect(tester.testChangePubKey(bob, 'DAI', false)).to.be.fulfilled;
        });
    });
});

