import { expect, use } from 'chai';
import { utils } from 'ethers';
import { Wallet } from 'zksync';
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
 * Tests for the CREATE2 accounts.
 */
describe(`CREATE2 tests`, () => {
    const transport = 'HTTP';
    const providerType = 'REST';
    const token = 'wBTC';
    let hilda: Wallet;
    let frida: Wallet;
    let david: Wallet;

    let tester: Tester;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', transport, providerType);
        david = await tester.fundedWallet('1.0');
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    step('should setup a create2 account', async () => {
        hilda = await tester.create2Wallet();
        await tester.testDeposit(hilda, token, DEPOSIT_AMOUNT, true);
        const cpk = await hilda.setSigningKey({
            feeToken: token,
            ethAuthType: 'CREATE2'
        });
        await cpk.awaitReceipt();
        const accountState = await hilda.getAccountState();
        expect(accountState.accountType, 'Incorrect account type').to.be.eql('CREATE2');
    });

    step('should make transfers from create2 account', async () => {
        await tester.testTransfer(hilda, david, token, TX_AMOUNT);
        await tester.testBatch(hilda, david, token, TX_AMOUNT);
    });

    step('should set pubkey and transfer in single batch', async () => {
        frida = await tester.create2Wallet();
        await tester.testDeposit(frida, token, DEPOSIT_AMOUNT, true);
        await tester.testCreate2CPKandTransfer(frida, david, token, TX_AMOUNT);
    });

    step('should fail eth-signed tx from create2 account', async () => {
        await tester.testCreate2TxFail(hilda, david, token, TX_AMOUNT);
    });

    step('should fail eth-signed batch from create2 account', async () => {
        // here we have a signle eth signature for the whole batch
        await tester.testCreate2SignedBatchFail(hilda, david, token, TX_AMOUNT);

        // Switch provider to RPC, because REST provider always expects
        // Ethereum signed message for the whole batch, skip this test.
        const oldProvider = tester.syncWallet.provider;
        tester.syncWallet.provider = await Tester.createSyncProvider('localhost', 'HTTP', 'RPC');
        await tester.testCreate2BatchFail(hilda, david, token, TX_AMOUNT);
        // Restore provider in tester
        tester.syncWallet.provider = oldProvider;
    });
});
