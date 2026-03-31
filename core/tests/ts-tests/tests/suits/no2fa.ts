import { expect, use } from 'chai';
import { utils } from 'ethers';
import { Wallet, crypto, Signer, No2FAWalletSigner } from 'zksync';
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
 * Tests for No2FA accounts.
 */
describe(`No2FA tests`, () => {
    const transport = 'HTTP';
    const providerType = 'REST';
    const token = 'wBTC';
    const secondToken = 'ETH';

    let tester: Tester;
    let hilda: Wallet;
    let hildaWithEthSigner: Wallet;
    let frida: Wallet;

    // The private key which won't require 2FA
    let zkPrivateKey: Uint8Array;
    // The private key which will require 2FA
    let zkPrivateKeyWith2FA: Uint8Array;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', transport, providerType);
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    step('should setup an account without 2fa', async () => {
        // The 2FA will be off only for this L2 private key
        zkPrivateKey = await crypto.privateKeyFromSeed(utils.randomBytes(32));
        zkPrivateKeyWith2FA = await crypto.privateKeyFromSeed(utils.randomBytes(32));

        // Even the wallets with no 2fa should sign message for CPK with their private key
        hilda = await tester.fundedWallet('1.0');
        frida = await tester.fundedWallet('1.0');
        hilda.signer = Signer.fromPrivateKey(zkPrivateKeyWith2FA);
        await tester.testDeposit(hilda, token, DEPOSIT_AMOUNT, true);
        await tester.testDeposit(frida, token, DEPOSIT_AMOUNT, true);
        await (
            await hilda.setSigningKey({
                feeToken: token,
                ethAuthType: 'ECDSA'
            })
        ).awaitReceipt();
        await (
            await frida.setSigningKey({
                feeToken: token,
                ethAuthType: 'ECDSA'
            })
        ).awaitReceipt();
        const pubKeyHash = await crypto.privateKeyToPubKeyHash(zkPrivateKey);
        await hilda.toggle2FA(false, pubKeyHash);

        const accountState = await hilda.getAccountState();
        expect(accountState.accountType, 'Incorrect account type').to.be.eql({
            No2FA: pubKeyHash
        });
    });

    step('Test No2FA with wrong PubKeyHash', async () => {
        hildaWithEthSigner = hilda;

        // Making sure that the wallet has no Ethereum private key
        // but has wrong l2 private key
        hilda = await Wallet.fromSyncSigner(
            new No2FAWalletSigner(hilda.address(), hilda.ethSigner().provider),
            Signer.fromPrivateKey(zkPrivateKeyWith2FA),
            hilda.provider
        );

        // Here the transfer without Ethereum signature, but with wrong l2 private key
        await expect(tester.testTransfer(hilda, frida, token, TX_AMOUNT)).to.be.rejected;

        // Now let's go back to the correct l2 private key
        hildaWithEthSigner.signer = Signer.fromPrivateKey(zkPrivateKey);
        await (
            await hildaWithEthSigner.setSigningKey({
                feeToken: token,
                ethAuthType: 'ECDSA'
            })
        ).awaitReceipt();

        // Making sure that hilda has correct signer
        hilda.signer = Signer.fromPrivateKey(zkPrivateKey);
    });

    step('Test No2FA transfers', async () => {
        await tester.testTransfer(hilda, frida, token, TX_AMOUNT);
        await tester.testBatch(hilda, frida, token, TX_AMOUNT);
        await tester.testBatchBuilderTransfersWithoutSignatures(hilda, frida, token, TX_AMOUNT);
    });

    step('Test No2FA Swaps', async () => {
        await tester.testDeposit(frida, secondToken, DEPOSIT_AMOUNT, true);
        await tester.testSwap(hilda, frida, token, secondToken, TX_AMOUNT);
    });

    step('Test No2FA Withdrawals', async () => {
        await tester.testWithdraw(hilda, token, TX_AMOUNT);
    });

    step('Switching 2FA on & providing PubKeyHash should fail', async () => {
        // Provide a PubKeyHash. Together with `enable: true` server is expected to return an error.
        const randomPrivateKey = await crypto.privateKeyFromSeed(utils.randomBytes(32));
        const randomPubKeyHash = await crypto.privateKeyToPubKeyHash(randomPrivateKey);
        let thrown = false;
        try {
            await hildaWithEthSigner.toggle2FA(true, randomPubKeyHash);
        } catch (e) {
            thrown = true;
        }
        expect(thrown, "Request with 'enable: true' and PubKeyHash provided was processed by server").to.be.true;

        // Account type should not change.
        const expectedPubKeyHash = await crypto.privateKeyToPubKeyHash(zkPrivateKey);
        const accountState = await hilda.getAccountState();
        expect(accountState.accountType, 'Incorrect account type').to.be.eql({
            No2FA: expectedPubKeyHash
        });
    });

    step('Test switching 2FA on', async () => {
        await hildaWithEthSigner.toggle2FA(true);
        const accountState = await hilda.getAccountState();
        expect(accountState.accountType, 'Incorrect account type').to.be.eql('Owned');
    });
});
