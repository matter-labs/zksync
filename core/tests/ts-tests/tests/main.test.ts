import { expect } from 'chai';
import { BigNumber, utils } from 'ethers';
import { Wallet, types } from 'zksync';

import { Tester } from './tester';
import './priority-ops';
import './change-pub-key';
import './transfer';
import './withdraw';
import './forced-exit';
import './misc';

const TX_AMOUNT = utils.parseEther('10.0');
// should be enough for ~200 test transactions (excluding fees), increase if needed
const DEPOSIT_AMOUNT = TX_AMOUNT.mul(200);

// prettier-ignore
const TestSuite = (token: types.TokenSymbol, transport: 'HTTP' | 'WS') =>
describe(`ZkSync integration tests (token: ${token}, transport: ${transport})`, () => {
    let tester: Tester;
    let alice: Wallet;
    let bob: Wallet;
    let operatorBalance: BigNumber;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', transport);
        alice = await tester.fundedWallet('5.0');
        bob = await tester.emptyWallet();
        operatorBalance = await tester.operatorBalance(token);
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    step('should execute an auto-approved deposit', async () => {
        await tester.testDeposit(alice, token, DEPOSIT_AMOUNT, true);
    });

    step('should execute a normal deposit', async () => {
        if (token == 'ETH') {
            await tester.testDeposit(alice, token, DEPOSIT_AMOUNT);
        } else {
            expect(await tester.syncWallet.isERC20DepositsApproved(token), 'Token should not be approved').to.be.false;
            const approveERC20 = await tester.syncWallet.approveERC20TokenDeposits(token);
            await approveERC20.wait();
            expect(await tester.syncWallet.isERC20DepositsApproved(token), 'Token should be approved').to.be.true;
            await tester.testDeposit(alice, token, DEPOSIT_AMOUNT);
            expect(await tester.syncWallet.isERC20DepositsApproved(token), 'Token should still be approved').to.be.true;
        }
    });

    step('should change pubkey onchain', async () => {
        await tester.testChangePubKey(alice, token, true);
    });

    step('should execute a transfer to new account', async () => {
        await tester.testTransfer(alice, bob, token, TX_AMOUNT);
    });

    step('should execute a transfer to existing account', async () => {
        await tester.testTransfer(alice, bob, token, TX_AMOUNT);
    });

    it('should execute a transfer to self', async () => {
        await tester.testTransfer(alice, alice, token, TX_AMOUNT);
    });

    step('should change pubkey offchain for alice', async () => {
        await tester.testChangePubKey(alice, token, false);
    });

    step('should test multi-transfers', async () => {
        await tester.testBatch(alice, bob, token, TX_AMOUNT);
        await tester.testIgnoredBatch(alice, bob, token, TX_AMOUNT);
        await tester.testFailedBatch(alice, bob, token, TX_AMOUNT);
    });

    step('should execute a withdrawal', async () => {
        await tester.testVerifiedWithdraw(alice, token, TX_AMOUNT);
    });

    step('should execute a ForcedExit', async () => {
        await tester.testVerifiedForcedExit(alice, bob, token);
    });

    it('should check collected fees', async () => {
        const collectedFee = (await tester.operatorBalance(token)).sub(operatorBalance);
        expect(collectedFee.eq(tester.runningFee), 'Fee collection failed').to.be.true;
    });

    it('should fail trying to send tx with wrong signature', async () => {
        await tester.testWrongSignature(alice, bob, token, TX_AMOUNT);
    });

    describe('Full Exit tests', () => {
        let carl: Wallet;

        before('create a test wallet', async () => {
            carl = await tester.fundedWallet('5.0');
        });

        step('should execute full-exit on random wallet', async () => {
            await tester.testFullExit(carl, token, 145);
        });

        step('should fail full-exit with wrong eth-signer', async () => {
            // make a deposit so that wallet is assigned an accountId
            await tester.testDeposit(carl, token, DEPOSIT_AMOUNT, true);

            const oldSigner = carl.ethSigner;
            carl.ethSigner = tester.ethWallet;
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), "Balance before Full Exit must be non-zero").to.be.false;
            expect(before.eq(after), "Balance after incorrect Full Exit should not change").to.be.true;
            carl.ethSigner = oldSigner;
        });

        step('should execute a normal full-exit', async () => {
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), "Balance before Full Exit must be non-zero").to.be.false;
            expect(after.eq(0), "Balance after Full Exit must be zero").to.be.true;
        });

        step('should execute full-exit on an empty wallet', async () => {
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), "Balance before Full Exit must be zero (we've already withdrawn all the funds)").to.be.true;
            expect(after.eq(0), "Balance after Full Exit must be zero").to.be.true;
        });
    });
});

// wBTC is chosen because it has decimals different from ETH (8 instead of 18).
// Using this token will help us to detect decimals-related errors.
const defaultERC20 = 'wBTC';

let tokenAndTransport = [];
if (process.env.TEST_TRANSPORT) {
    if (process.env.TEST_TOKEN) {
        // Both transport and token are set, use config from env.
        const envTransport = process.env.TEST_TRANSPORT.toUpperCase();
        const envToken = process.env.TEST_TOKEN.toUpperCase();
        tokenAndTransport = [
            {
                transport: envTransport,
                token: envToken
            }
        ];
    } else {
        // Only transport is set, use wBTC as default token for this transport.
        const envTransport = process.env.TEST_TRANSPORT.toUpperCase();
        tokenAndTransport = [
            {
                transport: envTransport,
                token: defaultERC20
            }
        ];
    }
} else {
    // Default case: run HTTP&ETH / WS&wBTC.
    tokenAndTransport = [
        {
            transport: 'HTTP',
            token: 'ETH'
        },
        {
            transport: 'WS',
            token: defaultERC20
        }
    ];
}

for (const input of tokenAndTransport) {
    // @ts-ignore
    TestSuite(input.token, input.transport);
}
