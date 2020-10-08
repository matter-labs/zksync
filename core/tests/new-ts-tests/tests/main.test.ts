import { expect, use } from 'chai';
// import promised from 'chai-as-promised';
import { BigNumber, utils } from 'ethers';
import { Wallet } from 'zksync';

import { Tester } from './tester';
import './priority-ops';
import './change-pub-key';
import './transfer';
import './withdraw';
import './misc';

// use(promised);

describe('ZkSync integration tests', () => {
    let tester: Tester;
    const one = utils.parseEther('1.0');
    let alice: Wallet;
    let bob: Wallet;
    let operatorBalance: BigNumber;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', 'HTTP');
        alice = await tester.fundedWallet('1.45');
        bob = await tester.emptyWallet();
        operatorBalance = await tester.operatorBalance('ETH');
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    step('should execute an auto-approved deposit', async () => {
        await tester.testDeposit(alice, 'ETH', one, true);
    });

    step('should execute a normal deposit', async () => {
        await tester.testDeposit(alice, 'ETH', one);

        // expect(await tester.syncWallet.isERC20DepositsApproved('DAI'), "Token should not be approved").to.be.false;
        // const approveERC20 = await tester.syncWallet.approveERC20TokenDeposits('DAI');
        // await approveERC20.wait();
        // expect(await tester.syncWallet.isERC20DepositsApproved('DAI'), "Token should be approved").to.be.true;
        // await expect(tester.testDeposit(bob, 'DAI', hundred)).to.be.fulfilled;
        // expect(await tester.syncWallet.isERC20DepositsApproved('DAI'), "Token should still be approved").to.be.true;
    });

    step('should change pubkey onchain', async () => {
        await tester.testChangePubKey(alice, 'ETH', true);
    });

    step('should execute a transfer to new account', async () => {
        await tester.testTransfer(alice, bob, 'ETH', one.div(10));
    });

    step('should execute a transfer to existing account', async () => {
        await tester.testTransfer(alice, bob, 'ETH', one.div(10));
    });

    it('should execute a transfer to self', async () => {
        await tester.testTransfer(alice, alice, 'ETH', one.div(10));
    });

    step('should change pubkey offchain', async () => {
        await tester.testChangePubKey(alice, 'ETH', false);
        await tester.testChangePubKey(bob, 'ETH', false);
    });

    step('should test multi-transfers', async () => {
        await tester.testMultiTransfer(alice, bob, 'ETH', one.div(100));
        await tester.testFailedMultiTransfer(alice, bob, 'ETH', one.div(100));
    });

    it('should fail trying to send tx with wrong signature', async () => {
        await tester.testWrongSignature(alice, bob);
    });

    step('should execute a withdrawal', async () => {
        await tester.testVerifiedWithdraw(alice, 'ETH', one.div(10));
    });

    step('should execute a fast withdrawal', async () => {
        await tester.testVerifiedWithdraw(bob, 'ETH', one.div(10), true);
    });

    it('should check collected fees', async () => {
        const collectedFee = (await tester.operatorBalance('ETH')).sub(operatorBalance);
        expect(collectedFee.eq(tester.runningFee), 'Fee collection failed').to.be.true;
    });

    describe('Full Exit tests', () => {
        let carl: Wallet;

        before('create a test wallet', async () => {
            carl = await tester.fundedWallet('0.5');
        });

        step('should execute full-exit on random wallet', async () => {
            await tester.testFullExit(carl, 'ETH', 145);
        });

        step('should fail full-exit with wrong eth-signer', async () => {
            // make a deposit so that wallet is assigned an accountId
            await tester.testDeposit(carl, 'ETH', one);

            let oldSigner = carl.ethSigner;
            carl.ethSigner = tester.ethWallet;
            let [before, after] = await tester.testFullExit(carl, 'ETH');
            expect(before.eq(0)).to.be.false;
            expect(before.eq(after)).to.be.true;
            carl.ethSigner = oldSigner;
        });

        step('should execute a normal full-exit', async () => {
            let [before, after] = await tester.testFullExit(carl, 'ETH');
            expect(before.eq(0)).to.be.false;
            expect(after.eq(0)).to.be.true;
        });

        step('should execute full-exit on an empty wallet', async () => {
            let [before, after] = await tester.testFullExit(carl, 'ETH');
            expect(before.eq(0)).to.be.true;
            expect(after.eq(0)).to.be.true;
        });
    });

    // step('...', async () => {
    //     await expect(tester.testTransactionResending(alice, carl)).to.be.fulfilled;
    // })
});
