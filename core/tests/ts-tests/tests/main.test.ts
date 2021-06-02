import { expect } from 'chai';
import { BigNumber, utils } from 'ethers';
import { Wallet, types } from 'zksync';

import { Tester } from './tester';
import './priority-ops';
import './change-pub-key';
import './transfer';
import './withdraw';
import './mint-nft';
import './forced-exit';
import './misc';
import './batch-builder';
import './create2';
import './swap';
import './register-factory';

const TX_AMOUNT = utils.parseEther('10.0');
// should be enough for ~200 test transactions (excluding fees), increase if needed
const DEPOSIT_AMOUNT = TX_AMOUNT.mul(200);

// prettier-ignore
/// We don't want to run tests with all tokens, so we highlight basic operations such as: Deposit, Withdrawal, Forced Exit
/// We want to check basic operations with all tokens, and other operations only if it's necessary
const TestSuite = (token: types.TokenSymbol, transport: 'HTTP' | 'WS', onlyBasic: boolean = false) =>
describe(`ZkSync integration tests (token: ${token}, transport: ${transport})`, () => {
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
        tester = await Tester.init('localhost', transport);
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

    step('should execute a normal deposit', async () => {
        if (token == 'ETH') {
            await tester.testDeposit(alice, token, DEPOSIT_AMOUNT);
        } else {
            expect(await tester.syncWallet.isERC20DepositsApproved(token), 'Token should not be approved').to.be
                .false;
            const approveERC20 = await tester.syncWallet.approveERC20TokenDeposits(token, DEPOSIT_AMOUNT);
            await approveERC20.wait();
            expect(
                await tester.syncWallet.isERC20DepositsApproved(token, DEPOSIT_AMOUNT),
                'Token should be approved'
            ).to.be.true;
            await tester.testDeposit(alice, token, DEPOSIT_AMOUNT);
            // It should not be approved because we have approved only DEPOSIT_AMOUNT, not the maximum possible amount of deposit
            expect(
                await tester.syncWallet.isERC20DepositsApproved(token, DEPOSIT_AMOUNT),
                'Token should not be approved'
            ).to.be.false;
            const approveERC20Next = await tester.syncWallet.approveERC20TokenDeposits(token);
            await approveERC20Next.wait();
            expect(await tester.syncWallet.isERC20DepositsApproved(token), 'The second deposit should be approved')
                .to.be.true;
        }
    });

    step('should change pubkey onchain', async () => {
        await tester.testChangePubKey(alice, token, true);
    });

    step('should execute a transfer to new account', async () => {
        await tester.testTransfer(alice, chuck, token, TX_AMOUNT);
    });

    step('should execute a mintNFT', async () => {
        nft =  await tester.testMintNFT(alice, chuck,  token);
    });
    step('should execute a getNFT', async () => {
        if (onlyBasic) {
            return
        }
        await tester.testGetNFT(alice, token);
    }).timeout(500000);

    step('should execute a transfer to existing account', async () => {
        if (onlyBasic) {
            return;
        }
        await tester.testTransfer(alice, chuck, token, TX_AMOUNT);
    });

    it('should execute a transfer to self', async () => {
        if (onlyBasic) {
            return;
        }
        await tester.testTransfer(alice, alice, token, TX_AMOUNT);
    });

    step('should change pubkey offchain', async () => {
        await tester.testChangePubKey(chuck, token, false);
    });

    step('should test multi-transfers', async () => {
        await tester.testBatch(alice, bob, token, TX_AMOUNT);
        await tester.testIgnoredBatch(alice, bob, token, TX_AMOUNT);
        await tester.testRejectedBatch(alice, bob, token, TX_AMOUNT);
        await tester.testInvalidFeeBatch(alice, bob, token, TX_AMOUNT);
    });

    step('should test batch-builder', async () => {
        // We will pay with different token.
        const feeToken = token == 'ETH' ? 'wBTC' : 'ETH';
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
        if (onlyBasic) {
            return;
        }
        const secondToken = token == 'ETH' ? 'wBTC' : 'ETH';
        await tester.testSwap(alice, frank, token, secondToken, TX_AMOUNT);
        await tester.testSwapBatch(alice, frank, david, token, secondToken, TX_AMOUNT);
    });

    step('should swap NFT for fungible tokens', async () => {
        if (onlyBasic) {
            return;
        }
        await tester.testSwapNFT(alice, chuck, token, nft.id, TX_AMOUNT);
    });

    step('should test multi-signers', async () => {
        // At this point, all these wallets already have their public keys set.
        await tester.testMultipleBatchSigners([alice, david, frank], token, TX_AMOUNT);
        await tester.testMultipleWalletsWrongSignature(alice, david, token, TX_AMOUNT);
    });

    step('should test backwards compatibility', async () => {
        await tester.testBackwardCompatibleEthMessages(alice, david, token, TX_AMOUNT);
    });

    step('should execute a withdrawal', async () => {
        await tester.testVerifiedWithdraw(alice, token, TX_AMOUNT);
    });

    step('should execute NFT transfer', async () => {
        if (onlyBasic) {
            return;
        }
        await tester.testTransferNFT(alice, chuck, token);
    });

    step('should execute NFT withdraw', async () => {
        await tester.testWithdrawNFT(chuck, token);
    });

    step('should execute a forced exit', async () => {
        await tester.testVerifiedForcedExit(alice, bob, token);
    });

    step('should register factory and withdraw nft', async () => {
        if (onlyBasic) {
            return;
        }
        await tester.testRegisterFactory(alice, token);
    });

    it('should check collected fees', async () => {
        const collectedFee = (await tester.operatorBalance(token)).sub(operatorBalance);
        expect(collectedFee.eq(tester.runningFee), `Fee collection failed, expected: ${tester.runningFee.toString()}, got: ${collectedFee.toString()}`).to.be.true;
    });

    it('should fail trying to send tx with wrong signature', async () => {
        if (onlyBasic) {
            return;
        }
        await tester.testWrongSignature(alice, bob, token, TX_AMOUNT);
    });

    describe('Full Exit tests', () => {
        let carl: Wallet;

        before('create a test wallet', async () => {
            carl = await tester.fundedWallet('5.0');
        });

        step('should execute full-exit on random wallet', async () => {
            if (onlyBasic) {
                return;
            }
            await tester.testFullExit(carl, token, 145);
        });

        step('should fail full-exit with wrong eth-signer', async () => {
            if (onlyBasic) {
                return;
            }
            // make a deposit so that wallet is assigned an accountId
            await tester.testDeposit(carl, token, DEPOSIT_AMOUNT, true);

            const oldSigner = carl.ethSigner;
            carl.ethSigner = tester.ethWallet;
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), 'Balance before Full Exit must be non-zero').to.be.false;
            expect(before.eq(after), 'Balance after incorrect Full Exit should not change').to.be.true;
            carl.ethSigner = oldSigner;
        });

        step('should execute NFT full-exit', async () => {
            if (onlyBasic) {
                return;
            }
            await tester.testMintNFT(alice, carl, token, true);
            await tester.testFullExitNFT(carl);
        });

        step('should execute a normal full-exit', async () => {
            if (onlyBasic) {
                return;
            }
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), 'Balance before Full Exit must be non-zero').to.be.false;
            expect(after.eq(0), 'Balance after Full Exit must be zero').to.be.true;
        });

        step('should execute full-exit on an empty wallet', async () => {
            if (onlyBasic) {
                return;
            }
            const [before, after] = await tester.testFullExit(carl, token);
            expect(before.eq(0), "Balance before Full Exit must be zero (we've already withdrawn all the funds)").to
                .be.true;
            expect(after.eq(0), 'Balance after Full Exit must be zero').to.be.true;
        });
    });

    describe('CREATE2 tests', () => {
        let hilda: Wallet;
        let frida: Wallet;

        step('should setup a create2 account', async () => {
            if (onlyBasic) {
                return;
            }
            hilda = await tester.create2Wallet();
            await tester.testDeposit(hilda, token, DEPOSIT_AMOUNT, true);
            const cpk = await hilda.setSigningKey({
                feeToken: token,
                ethAuthType: 'CREATE2'
            });
            await cpk.awaitReceipt();
        });

        step('should make transfers from create2 account', async () => {
            if (onlyBasic) {
                return;
            }
            await tester.testTransfer(hilda, david, token, TX_AMOUNT);
            await tester.testBatch(hilda, david, token, TX_AMOUNT);
        });

        step('should set pubkey and transfer in single batch', async () => {
            if (onlyBasic) {
                return;
            }
            frida = await tester.create2Wallet();
            await tester.testDeposit(frida, token, DEPOSIT_AMOUNT, true);
            await tester.testCreate2CPKandTransfer(frida, david, token, TX_AMOUNT);
        });

        step('should fail eth-signed tx from create2 account', async () => {
            if (onlyBasic) {
                return;
            }
            await tester.testCreate2TxFail(hilda, david, token, TX_AMOUNT);
        });

        step('should fail eth-signed batch from create2 account', async () => {
            if (onlyBasic) {
                return;
            }
            // here we have a signle eth signature for the whole batch
            await tester.testCreate2SignedBatchFail(hilda, david, token, TX_AMOUNT);
            // here the only each individual transaction is signed
            await tester.testCreate2BatchFail(hilda, david, token, TX_AMOUNT);
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
        const envToken = process.env.TEST_TOKEN;
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
    // Default case: run HTTP&ETH / HTTP&wBTC.
    tokenAndTransport = [
        {
            transport: 'HTTP',
            token: 'ETH',
            onlyBasic: true
        },
        {
            transport: 'HTTP',
            token: defaultERC20,
            onlyBasic: false
        }
    ];
}

for (const input of tokenAndTransport) {
    // @ts-ignore
    TestSuite(input.token, input.transport, input.onlyBasic);
}
