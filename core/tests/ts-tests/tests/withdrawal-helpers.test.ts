import { Wallet } from 'zksync';
import { Tester } from './tester';
import { utils } from 'ethers';
import './priority-ops';
import './change-pub-key';
import './withdrawal-helpers';
import './forced-exit-requests';

import { loadTestConfig } from 'reading-tool';

const TX_AMOUNT = utils.parseEther('0.1');
const DEPOSIT_AMOUNT = TX_AMOUNT.mul(2000);

const TEST_CONFIG = loadTestConfig(true);

// The token here should have the ERC20 implementation from RevertTransferERC20.sol
const erc20Token = 'wBTC';

describe('Withdrawal helpers tests', () => {
    let tester: Tester;
    let alice: Wallet;
    let bob: Wallet;
    let chuck: Wallet;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', 'HTTP');
        alice = await tester.fundedWallet('10.0');
        bob = await tester.fundedWallet('10.0');
        chuck = await tester.emptyWallet();

        for (const token of ['ETH', erc20Token]) {
            await tester.testDeposit(alice, token, DEPOSIT_AMOUNT, true);
            await tester.testChangePubKey(alice, token, false);
        }

        // This is needed to interact with blockchain
        alice.ethSigner.connect(tester.ethProvider);
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    it('should recover failed ETH withdraw', async () => {
        await tester.testRecoverETHWithdrawal(alice, TEST_CONFIG.withdrawalHelpers.revert_receive_address, TX_AMOUNT);
    });

    it('should recover failed ERC20 withdraw', async () => {
        await tester.testRecoverERC20Withdrawal(
            alice,
            TEST_CONFIG.withdrawalHelpers.revert_receive_address,
            erc20Token,
            TX_AMOUNT
        );
    });

    it('should recover multiple withdrawals', async () => {
        await tester.testRecoverMultipleWithdrawals(
            alice,
            [
                TEST_CONFIG.withdrawalHelpers.revert_receive_address,
                TEST_CONFIG.withdrawalHelpers.revert_receive_address
            ],
            ['ETH', erc20Token],
            [TX_AMOUNT, TX_AMOUNT]
        );
    });

    it('forced_exit_request should recover mutiple tokens', async () => {
        await tester.testForcedExitRequestMultipleTokens(
            alice,
            bob.ethSigner,
            chuck.address(),
            ['ETH', erc20Token],
            [TX_AMOUNT, TX_AMOUNT.mul(2)]
        );
    });
});
