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
