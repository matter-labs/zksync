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

describe(`Extended tests`, () => {
    const transport = 'HTTP';
    const providerType = 'REST';
    const token = 'wBTC';
    const secondToken = 'ETH';

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

    step('should execute a manually approved deposit', async () => {
        // Start by setting approved amount to 0.
        const resetApproveERC20 = await tester.syncWallet.approveERC20TokenDeposits(token, BigNumber.from(0));
        await resetApproveERC20.wait();

        // Now token must not be approved.
        expect(await tester.syncWallet.isERC20DepositsApproved(token), 'Token should not be approved').to.be.false;
        const approveERC20 = await tester.syncWallet.approveERC20TokenDeposits(token, DEPOSIT_AMOUNT);
        await approveERC20.wait();
        expect(await tester.syncWallet.isERC20DepositsApproved(token, DEPOSIT_AMOUNT), 'Token should be approved').to.be
            .true;
        await tester.testDeposit(alice, token, DEPOSIT_AMOUNT);
        // It should not be approved because we have approved only DEPOSIT_AMOUNT, not the maximum possible amount of deposit
        expect(await tester.syncWallet.isERC20DepositsApproved(token, DEPOSIT_AMOUNT), 'Token should not be approved')
            .to.be.false;
        const approveERC20Next = await tester.syncWallet.approveERC20TokenDeposits(token);
        await approveERC20Next.wait();
        expect(await tester.syncWallet.isERC20DepositsApproved(token), 'The second deposit should be approved').to.be
            .true;
    });

    step('should change pubkey onchain', async () => {
        await tester.testChangePubKey(alice, token, true);
    });

    step('should execute a mintNFT', async () => {
        // Recipient account must exist, so create it by performing a transfer.
        await tester.testTransfer(alice, chuck, token, TX_AMOUNT);

        nft = await tester.testMintNFT(alice, chuck, token);
    });

    step('should execute a getNFT', async () => {
        await tester.testGetNFT(alice, token);
    });

    step('should execute a transfer to existing account', async () => {
        await tester.testTransfer(alice, chuck, token, TX_AMOUNT);
    });

    it('should execute a transfer to self', async () => {
        await tester.testTransfer(alice, alice, token, TX_AMOUNT);
    });

    step('should test batch-builder', async () => {
        // We will pay with different token.
        const feeToken = secondToken;
        // Add these accounts to the network.
        await tester.testTransfer(alice, david, token, TX_AMOUNT.mul(10));
        await tester.testTransfer(alice, judy, token, TX_AMOUNT.mul(10));
        await tester.testTransfer(alice, frank, token, TX_AMOUNT.mul(10));
        await tester.testTransfer(alice, chris, token, TX_AMOUNT.mul(10));

        // Also deposit another token to pay with.
        await tester.testDeposit(frank, feeToken, DEPOSIT_AMOUNT, true);

        await tester.testBatchBuilderInvalidUsage(david, alice, token);
        await tester.testBatchBuilderChangePubKey(david, token, TX_AMOUNT, true);
        await tester.testBatchBuilderSignedChangePubKey(chris, token, TX_AMOUNT);
        await tester.testBatchBuilderChangePubKey(frank, token, TX_AMOUNT, false);
        await tester.testBatchBuilderTransfers(david, frank, token, TX_AMOUNT);
        await tester.testBatchBuilderPayInDifferentToken(frank, david, token, feeToken, TX_AMOUNT);
        await tester.testBatchBuilderNFT(frank, david, token);
        // Finally, transfer, withdraw and forced exit in a single batch.
        await tester.testBatchBuilderGenericUsage(david, frank, judy, token, TX_AMOUNT);
    });

    step('should test swaps and limit orders', async () => {
        await tester.testSwap(alice, frank, token, secondToken, TX_AMOUNT);
        await tester.testSwapBatch(alice, frank, david, token, secondToken, TX_AMOUNT);
        await tester.testSwapMissingSignatures(alice, frank, token, secondToken, TX_AMOUNT);
    });

    step('should swap NFT for fungible tokens', async () => {
        await tester.testChangePubKey(chuck, token, false);

        await tester.testSwapNFT(alice, chuck, token, nft.id, TX_AMOUNT);
    });

    step('should test backwards compatibility', async () => {
        await tester.testBackwardCompatibleEthMessages(alice, david, token, TX_AMOUNT);
    });

    step('should execute NFT transfer', async () => {
        await tester.testTransferNFT(alice, chuck, token);
    });

    step('should execute NFT withdraw', async () => {
        await tester.testWithdrawNFT(chuck, token);
    });

    step('should register factory and withdraw nft', async () => {
        await tester.testRegisterFactory(alice, token);
    });

    it('should check collected fees', async () => {
        const collectedFee = (await tester.operatorBalance(token)).sub(operatorBalance);
        expect(
            collectedFee.eq(tester.runningFee),
            `Fee collection failed, expected: ${tester.runningFee.toString()}, got: ${collectedFee.toString()}`
        ).to.be.true;
    });

    it('should fail trying to send tx with wrong signature', async () => {
        await tester.testWrongSignature(alice, bob, token, TX_AMOUNT, providerType);
    });
});
