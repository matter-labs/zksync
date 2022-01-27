import { expect, use } from 'chai';
import { utils } from 'ethers';
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

// wBTC is chosen because it has decimals different from ETH (8 instead of 18).
// Using this token will help us to detect decimals-related errors.
const defaultERC20 = 'wBTC';

/**
 * Tests for the Full Exit priority operation.
 * These tests don't actually rely on the API, so they are not included into the basic suite,
 * but the processing of ERC20 tokens and ETH in server and contract is different, thus we run
 * these tests for both types of tokens.
 */
const FullExitTestSuite = (token: types.TokenSymbol) =>
    describe(`Full Exit tests`, () => {
        const transport = 'HTTP';
        const providerType = 'REST';

        let tester: Tester;
        let alice: Wallet;
        let carl: Wallet;

        before('create tester and test wallets', async () => {
            tester = await Tester.init('localhost', transport, providerType);
            alice = await tester.fundedWallet('5.0');
            carl = await tester.fundedWallet('5.0');
        });

        after('disconnect tester', async () => {
            await tester.disconnect();
        });

        step('should execute full-exit on random wallet', async () => {
            await tester.testFullExit(carl, token, 145);
        });

        step('should fail full-exit with wrong eth-signer', async () => {
            // make a deposit so that wallet is assigned an accountId
            await tester.testDeposit(carl, token, DEPOSIT_AMOUNT, true);

            const oldSigner = carl._ethSigner;
            carl._ethSigner = tester.ethWallet;
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), 'Balance before Full Exit must be non-zero').to.be.false;
            expect(before.eq(after), 'Balance after incorrect Full Exit should not change').to.be.true;
            carl._ethSigner = oldSigner;
        });

        step('should execute NFT full-exit', async () => {
            await tester.testDeposit(alice, token, DEPOSIT_AMOUNT, true);
            await tester.testChangePubKey(alice, token, false);

            await tester.testMintNFT(alice, carl, token, true);
            await tester.testFullExitNFT(carl);
        });

        step('should execute a normal full-exit', async () => {
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), 'Balance before Full Exit must be non-zero').to.be.false;
            expect(after.eq(0), 'Balance after Full Exit must be zero').to.be.true;
        });

        step('should execute full-exit on an empty wallet', async () => {
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), "Balance before Full Exit must be zero (we've already withdrawn all the funds)").to.be
                .true;
            expect(after.eq(0), 'Balance after Full Exit must be zero').to.be.true;
        });
    });

FullExitTestSuite(defaultERC20);
FullExitTestSuite('ETH');
