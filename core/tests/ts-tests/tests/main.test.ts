import { expect } from 'chai';
import { BigNumber, utils, ethers } from 'ethers';
import { Wallet, types, crypto, Signer, No2FAWalletSigner } from 'zksync';

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
import './token-listing';

const TX_AMOUNT = utils.parseEther('10.0');
// should be enough for ~200 test transactions (excluding fees), increase if needed
const DEPOSIT_AMOUNT = TX_AMOUNT.mul(200);

// prettier-ignore
/// We don't want to run tests with all tokens, so we highlight basic operations such as: Deposit, Withdrawal, Forced Exit
/// We want to check basic operations with all tokens, and other operations only if it's necessary
const TestSuite = (token: types.TokenSymbol, transport: 'HTTP' | 'WS', providerType: 'REST' | 'RPC', onlyBasic: boolean = false) =>
describe(`ZkSync integration tests (token: ${token}, transport: ${transport}, provider: ${providerType})`, () => {
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
        nft = await tester.testMintNFT(alice, chuck, token);
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
        await tester.testRejectedBatch(alice, bob, token, TX_AMOUNT, providerType);
        await tester.testInvalidFeeBatch(alice, bob, token, TX_AMOUNT, providerType);
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
        await tester.testSwapMissingSignatures(alice, frank, token, secondToken, TX_AMOUNT);
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
        await tester.testMultipleWalletsWrongSignature(alice, david, token, TX_AMOUNT, providerType);
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
        await tester.testWrongSignature(alice, bob, token, TX_AMOUNT, providerType);
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
            const accountState = await hilda.getAccountState();
            expect(accountState.accountType, 'Incorrect account type').to.be.eql('CREATE2');
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
            if(providerType === 'RPC') {
                // REST provider always expects Ethereum signed message for the whole batch, skip this test.
                await tester.testCreate2BatchFail(hilda, david, token, TX_AMOUNT);
            }
        });
    });

    describe('No2FA tests', () => {
        let hilda: Wallet;
        let hildaWithEthSigner: Wallet;
        let frida: Wallet;

        step('should setup an account without 2fa', async () => {
            if (onlyBasic) {
                return;
            }

            const zkPrivateKey = await crypto.privateKeyFromSeed(utils.arrayify(ethers.constants.HashZero))
            // Even the wallets with no 2fa should sign message for CPK with their private key
            hilda = await tester.fundedWallet('1.0');
            frida = await tester.fundedWallet('1.0');
            hilda.signer = Signer.fromPrivateKey(zkPrivateKey);
            await tester.testDeposit(hilda, token, DEPOSIT_AMOUNT, true);
            await tester.testDeposit(frida, token, DEPOSIT_AMOUNT, true);
            await (await hilda.setSigningKey({
                feeToken: token,
                ethAuthType: 'ECDSA'
            })).awaitReceipt();
            await (await frida.setSigningKey({
                feeToken: token,
                ethAuthType: 'ECDSA'
            })).awaitReceipt();
            await hilda.toggle2FA(false);

            const accountState = await hilda.getAccountState();
            expect(accountState.accountType, 'Incorrect account type').to.be.eql('No2FA');

            hildaWithEthSigner = hilda;
            // Making sure that the wallet has no Ethereum private key
            const ethSigner = new No2FAWalletSigner(hilda.address(), hilda.ethSigner.provider);
            const syncSigner = Signer.fromPrivateKey(zkPrivateKey);
            hilda = await Wallet.fromSyncSigner(
                ethSigner,
                syncSigner,
                hilda.provider
            );
        });

        step('Test No2FA transfers', async () => {
            if (onlyBasic) {
                return;
            }

           await tester.testTransfer(hilda, frida, token, TX_AMOUNT);
           await tester.testBatch(hilda, frida, token, TX_AMOUNT);
           await tester.testBatchBuilderTransfersWithoutSignatures(hilda, frida, token, TX_AMOUNT);
        })

        step('Test No2FA Swaps', async () => {
            if(onlyBasic) {
                return;
            }

            const secondToken = token == 'ETH' ? 'wBTC' : 'ETH';
            await tester.testDeposit(frida, secondToken, DEPOSIT_AMOUNT, true);
            await tester.testSwap(hilda, frida, token, secondToken, TX_AMOUNT);
        })

        step('Test No2FA Withdrawals', async () => {
            if(onlyBasic) {
                return;
            }

            await tester.testWithdraw(hilda, token, TX_AMOUNT);
        })

        step('Test switching 2FA on', async () => {
            if(onlyBasic) {
                return;
            }

            await hildaWithEthSigner.toggle2FA(true);
            const accountState = await hilda.getAccountState();
            expect(accountState.accountType, 'Incorrect account type').to.be.eql('Owned');
        })
    });

    describe('Permissionless token listing tests', () => {
        step('Test ERC20 token listing', async () => {
            if(onlyBasic || providerType == 'RPC') {
                return;
            }
            await tester.testERC20Listing();
        })
        step('Test non-ERC20 token listing', async () => {
            if(onlyBasic || providerType == 'RPC') {
                return;
            }
            await tester.testNonERC20Listing();
        })
    })
});

// wBTC is chosen because it has decimals different from ETH (8 instead of 18).
// Using this token will help us to detect decimals-related errors.
const defaultERC20 = 'wBTC';
const defaultProviderType = 'REST';

let tokenAndTransport = [];
if (process.env.TEST_TRANSPORT) {
    if (process.env.TEST_TOKEN) {
        // Both transport and token are set, use config from env.
        const envTransport = process.env.TEST_TRANSPORT.toUpperCase();
        const envToken = process.env.TEST_TOKEN;
        tokenAndTransport = [
            {
                transport: envTransport,
                token: envToken,
                providerType: process.env.TEST_PROVIDER ? process.env.TEST_PROVIDER : defaultProviderType
            }
        ];
    } else {
        // Only transport is set, use wBTC as default token for this transport.
        const envTransport = process.env.TEST_TRANSPORT.toUpperCase();
        tokenAndTransport = [
            {
                transport: envTransport,
                token: defaultERC20,
                providerType: process.env.TEST_PROVIDER ? process.env.TEST_PROVIDER : defaultProviderType
            }
        ];
    }
} else {
    // Default case: run HTTP&ETH / HTTP&wBTC.
    tokenAndTransport = [
        {
            transport: 'HTTP',
            token: 'ETH',
            providerType: 'RPC',
            onlyBasic: true
        },
        {
            transport: 'HTTP',
            token: defaultERC20,
            providerType: 'RPC',
            onlyBasic: false
        },
        {
            transport: 'HTTP',
            token: 'ETH',
            providerType: 'REST',
            onlyBasic: true
        },
        {
            transport: 'HTTP',
            token: defaultERC20,
            providerType: 'REST',
            onlyBasic: false
        }
    ];
}

for (const input of tokenAndTransport) {
    // @ts-ignore
    TestSuite(input.token, input.transport, input.providerType, input.onlyBasic);
}
