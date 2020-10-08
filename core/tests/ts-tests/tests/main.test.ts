import { expect } from 'chai';
import { BigNumber, utils } from 'ethers';
import { Wallet, types } from 'zksync';

import { Tester } from './tester';
import './priority-ops';
import './change-pub-key';
import './transfer';
import './withdraw';
import './misc';

// prettier-ignore
const TestSuite = (token: types.TokenSymbol, transport: 'HTTP' | 'WS') =>
describe(`ZkSync integration tests (token: ${token}, transport: ${transport})`, () => {
    let tester: Tester;
    const hundred = utils.parseEther('100.0');
    let alice: Wallet;
    let bob: Wallet;
    let operatorBalance: BigNumber;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', transport);
        alice = await tester.fundedWallet('145.0');
        bob = await tester.emptyWallet();
        operatorBalance = await tester.operatorBalance(token);
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    step('should execute an auto-approved deposit', async () => {
        await tester.testDeposit(alice, token, hundred, true);
    });

    step('should execute a normal deposit', async () => {
        if (token == 'ETH') {
            await tester.testDeposit(alice, token, hundred);
        } else {
            expect(await tester.syncWallet.isERC20DepositsApproved(token), 'Token should not be approved').to.be.false;
            const approveERC20 = await tester.syncWallet.approveERC20TokenDeposits(token);
            await approveERC20.wait();
            expect(await tester.syncWallet.isERC20DepositsApproved(token), 'Token should be approved').to.be.true;
            await tester.testDeposit(bob, token, hundred);
            expect(await tester.syncWallet.isERC20DepositsApproved(token), 'Token should still be approved').to.be.true;
        }
    });

    step('should change pubkey onchain', async () => {
        await tester.testChangePubKey(alice, token, true);
    });

    step('should execute a transfer to new account', async () => {
        await tester.testTransfer(alice, bob, token, hundred.div(10));
    });

    step('should execute a transfer to existing account', async () => {
        await tester.testTransfer(alice, bob, token, hundred.div(10));
    });

    it('should execute a transfer to self', async () => {
        await tester.testTransfer(alice, alice, token, hundred.div(10));
    });

    step('should change pubkey offchain', async () => {
        await tester.testChangePubKey(alice, token, false);
        await tester.testChangePubKey(bob, token, false);
    });

    step('should test multi-transfers', async () => {
        await tester.testBatch(alice, bob, token, hundred.div(100));
        await tester.testIgnoredBatch(alice, bob, token, hundred.div(100));
        await tester.testFailedBatch(alice, bob, token, hundred.div(100));
    });

    it('should fail trying to send tx with wrong signature', async () => {
        await tester.testWrongSignature(alice, bob);
    });

    step('should execute a withdrawal', async () => {
        await tester.testVerifiedWithdraw(alice, token, hundred.div(10));
    });

    step('should execute a fast withdrawal', async () => {
        await tester.testVerifiedWithdraw(bob, token, hundred.div(10), true);
    });

    it('should check collected fees', async () => {
        const collectedFee = (await tester.operatorBalance(token)).sub(operatorBalance);
        expect(collectedFee.eq(tester.runningFee), 'Fee collection failed').to.be.true;
    });

    describe('Full Exit tests', () => {
        let carl: Wallet;

        before('create a test wallet', async () => {
            carl = await tester.fundedWallet('0.5');
        });

        step('should execute full-exit on random wallet', async () => {
            await tester.testFullExit(carl, token, 145);
        });

        step('should fail full-exit with wrong eth-signer', async () => {
            // make a deposit so that wallet is assigned an accountId
            await tester.testDeposit(carl, token, hundred);

            let oldSigner = carl.ethSigner;
            carl.ethSigner = tester.ethWallet;
            let [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0)).to.be.false;
            expect(before.eq(after)).to.be.true;
            carl.ethSigner = oldSigner;
        });

        step('should execute a normal full-exit', async () => {
            let [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0)).to.be.false;
            expect(after.eq(0)).to.be.true;
        });

        step('should execute full-exit on an empty wallet', async () => {
            let [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0)).to.be.true;
            expect(after.eq(0)).to.be.true;
        });
    });

    // step('...', async () => {
    //     await expect(tester.testTransactionResending(alice, carl)).to.be.fulfilled;
    // })
});

for (const transport of ['HTTP', 'WS']) {
    for (const token of ['ETH', 'DAI']) {
        // @ts-ignore
        TestSuite(token, transport);
    }
}
