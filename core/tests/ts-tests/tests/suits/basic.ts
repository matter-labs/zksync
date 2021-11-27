import { expect, use } from 'chai';
import { BigNumber, utils } from 'ethers';
import { Wallet, types, crypto, Signer, No2FAWalletSigner } from 'zksync';
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

let tokenAndTransport = [
    {
        transport: 'HTTP',
        token: 'ETH',
        providerType: 'RPC'
    },
    {
        transport: 'HTTP',
        token: defaultERC20,
        providerType: 'RPC'
    },
    {
        transport: 'HTTP',
        token: 'ETH',
        providerType: 'REST'
    },
    {
        transport: 'HTTP',
        token: defaultERC20,
        providerType: 'REST'
    }
];

// prettier-ignore
/// We don't want to run tests with all tokens, so we highlight basic operations such as: Deposit, Withdrawal, Forced Exit
/// We want to check basic operations with all tokens, and other operations only if it's necessary
const BasicTestSuite = (token: types.TokenSymbol, transport: 'HTTP' | 'WS', providerType: 'REST' | 'RPC') =>
    describe(`ZkSync integration tests (token: ${token}, transport: ${transport}, provider: ${providerType})`, () => {
        let tester: Tester;
        let alice: Wallet;
        let bob: Wallet;
        let chuck: Wallet;
        let operatorBalance: BigNumber;

        before('create tester and test wallets', async () => {
            tester = await Tester.init('localhost', transport, providerType);
            alice = await tester.fundedWallet('5.0');
            bob = await tester.emptyWallet();
            chuck = await tester.emptyWallet();
            operatorBalance = await tester.operatorBalance(token);
        });

        after('disconnect tester', async () => {
            await tester.disconnect();
        });

        step('should execute an auto-approved deposit', async () => {
            await tester.testDeposit(alice, token, DEPOSIT_AMOUNT, true);
        });

        step('should change pubkey offchain', async () => {
            await tester.testChangePubKey(alice, token, false);
        });

        step('should execute a transfer to new account', async () => {
            await tester.testTransfer(alice, bob, token, TX_AMOUNT);
        });

        step('should execute a mintNFT', async () => {
            await tester.testMintNFT(alice, bob, token);
        });

        step('should execute a transfer to existing account', async () => {
            await tester.testTransfer(alice, bob, token, TX_AMOUNT);
        });

        step('should test multi-transfers', async () => {
            await tester.testBatch(alice, bob, token, TX_AMOUNT);
            await tester.testIgnoredBatch(alice, bob, token, TX_AMOUNT);
            await tester.testRejectedBatch(alice, bob, token, TX_AMOUNT, providerType);
            await tester.testInvalidFeeBatch(alice, bob, token, TX_AMOUNT, providerType);
        });

        step('should test multi-signers', async () => {
            await tester.testTransfer(alice, chuck, token, TX_AMOUNT);
            await tester.testChangePubKey(bob, token, false);
            await tester.testChangePubKey(chuck, token, false);

            await tester.testMultipleBatchSigners([alice, bob, chuck], token, TX_AMOUNT);
            await tester.testMultipleWalletsWrongSignature(alice, bob, token, TX_AMOUNT, providerType);
        });

        step('should test backwards compatibility', async () => {
            await tester.testBackwardCompatibleEthMessages(alice, bob, token, TX_AMOUNT);
        });

        step('should execute a withdrawal', async () => {
            await tester.testVerifiedWithdraw(alice, token, TX_AMOUNT);
        });

        step('should execute NFT withdraw', async () => {
            await tester.testWithdrawNFT(bob, token);
        });

        step('should execute a forced exit', async () => {
            const forcedExitWallet = await tester.emptyWallet();
            await tester.testTransfer(alice, forcedExitWallet, token, TX_AMOUNT);
            await tester.testVerifiedForcedExit(alice, forcedExitWallet, token);
        });

        it('should check collected fees', async () => {
            const collectedFee = (await tester.operatorBalance(token)).sub(operatorBalance);
            expect(collectedFee.eq(tester.runningFee), `Fee collection failed, expected: ${tester.runningFee.toString()}, got: ${collectedFee.toString()}`).to.be.true;
        });
    });

for (const input of tokenAndTransport) {
    // @ts-ignore
    BasicTestSuite(input.token, input.transport, input.providerType, input.onlyBasic);
}
