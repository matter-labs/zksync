import { expect, use } from 'chai';
import { BigNumber, utils } from 'ethers';
import { Wallet, types } from 'zksync';
import chaiAsPromised from 'chai-as-promised';
import { Tester } from '../tester/tester';
import '../tester/priority-ops';
import '../tester/change-pub-key';
import '../tester/transfer';
import '../tester/withdraw';
import '../tester/mint-nft';
import '../tester/forced-exit';
import '../tester/misc';
import '../tester/batch-builder';
import '../tester/create2';
import '../tester/swap';
import '../tester/register-factory';
import '../tester/token-listing';

use(chaiAsPromised);

const TX_AMOUNT = utils.parseEther('10.0');
// should be enough for ~200 test transactions (excluding fees), increase if needed
const DEPOSIT_AMOUNT = TX_AMOUNT.mul(200);

/**
 * Extended test suite.
 *
 * This suite contains tests that extend the concepts from the `basic` test suite, and don't actually need
 * to be repeated with every token/API combination.
 *
 * These tests are allowed to run longer than ones in `basic` test suite, but please try to keep them as fast
 * as possible without compromising on the test behavior and stability.
 */
describe.only(`Subsidy tests`, () => {
    const transport = 'HTTP';
    const providerType = 'RPC';
    const token = 'wBTC';

    let tester: Tester;
    let alice: Wallet;
    let bob: Wallet;
    let chuck: Wallet;
    let david: Wallet;
    let frank: Wallet;
    let judy: Wallet;
    let chris: Wallet;
    let operatorBalance: BigNumber;
    let nft: types.NFT;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', transport, providerType);
        alice = await tester.fundedWallet('5.0');
        bob = await tester.emptyWallet();
        chuck = await tester.emptyWallet();
        david = await tester.fundedWallet('1.0');
        frank = await tester.fundedWallet('1.0');
        judy = await tester.emptyWallet();
        chris = await tester.emptyWallet();
        operatorBalance = await tester.operatorBalance(token);
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    step('should execute an auto-approved deposit', async () => {
        await tester.testDeposit(alice, token, DEPOSIT_AMOUNT, true);
    });

    step('should change pubkey onchain', async () => {
        await tester.testChangePubKey(alice, token, true);
    });

    step('Should test subsidy', async () => {
        const wallet1 = await tester.create2Wallet();
        await tester.testTransfer(alice, wallet1, token, TX_AMOUNT);
        await tester.testSubsidyForCREATE2ChangePubKey(wallet1, token);

        const wallet2 = await tester.create2Wallet();
        await tester.testTransfer(alice, wallet2, token, TX_AMOUNT);
        await tester.testSubsidyForBatch(wallet2, token);
    });
});
