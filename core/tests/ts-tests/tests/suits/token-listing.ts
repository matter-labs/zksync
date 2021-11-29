import { use } from 'chai';
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

/**
 * Tests for the permissionless token listing procedure.
 */
describe(`Permissionless token listing tests`, () => {
    const transport = 'HTTP';
    const providerType = 'REST';

    let tester: Tester;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', transport, providerType);
    });

    after('disconnect tester', async () => {
        await tester.disconnect();
    });

    step('Test ERC20 token listing', async () => {
        await tester.testERC20Listing();
    });

    step('Test non-ERC20 token listing', async () => {
        await tester.testNonERC20Listing();
    });
});
