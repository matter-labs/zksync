import { Wallet, RestProvider, getDefaultRestProvider, types } from 'zksync';
import { Tester } from './tester';
import './priority-ops';
import './change-pub-key';
import './transfer';
import './withdraw';
import './forced-exit';
import { expect } from 'chai';

import * as api from './api';

describe('ZkSync REST API V0.1 tests', () => {
    let tester: Tester;
    let alice: Wallet;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', 'HTTP', 'RPC');
        alice = await tester.fundedWallet('1.0');
        let bob = await tester.emptyWallet();
        for (const token of ['ETH', 'DAI']) {
            const thousand = tester.syncProvider.tokenSet.parseToken(token, '1000');
            await tester.testDeposit(alice, token, thousand, true);
            await tester.testChangePubKey(alice, token, false);
            await tester.testTransfer(alice, bob, token, thousand.div(4));
            await tester.testForcedExit(alice, bob, token);
            await tester.testWithdraw(alice, token, thousand.div(5));
            await tester.testFullExit(alice, token);
            await tester.testDeposit(alice, token, thousand.div(10), true);
        }
        api.deleteUnusedGenFiles();
    });

    after('disconnect tester', async () => {
        api.deleteUnusedGenFiles();
        await tester.disconnect();
    });

    it('should check status response type', async () => {
        await api.checkStatusResponseType();
    });

    it('should check testnet config response type', async () => {
        await api.checkTestnetConfigResponseType();
    });

    it('should check withdrawal processing time response type', async () => {
        await api.checkWithdrawalProcessingTimeResponseType();
    });

    it('should check tx history response type', async () => {
        await api.checkTxHistoryResponseType(alice.address());
    });

    it('should check blocks response type', async () => {
        const blocksToCheck = 10;
        const blocks = await api.checkBlocksResponseType();
        for (const { block_number } of blocks.slice(-blocksToCheck)) {
            await api.checkBlockResponseType(block_number);
            const txs = await api.checkBlockTransactionsResponseType(block_number);
            for (const { tx_hash } of txs) {
                await api.checkTransactionsResponseType(tx_hash);
            }
        }
    });
});

describe('ZkSync REST API V0.2 tests', () => {
    let tester: Tester;
    let alice: Wallet;
    let bob: Wallet;
    let provider: RestProvider;
    let lastTxHash: string;
    let lastTxReceipt: types.TransactionReceipt;

    before('create tester and test wallets', async () => {
        provider = await getDefaultRestProvider('localhost');
        tester = await Tester.init('localhost', 'HTTP', 'REST');
        alice = await tester.fundedWallet('1.0');
        bob = await tester.emptyWallet();
        for (const token of ['ETH']) {
            const thousand = tester.syncProvider.tokenSet.parseToken(token, '1000');
            await tester.testDeposit(alice, token, thousand, true);
            await tester.testChangePubKey(alice, token, false);
            await tester.testTransfer(alice, bob, token, thousand.div(4));
        }

        const handle = await alice.syncTransfer({
            to: bob.address(),
            token: 'ETH',
            amount: alice.provider.tokenSet.parseToken('ETH', '1')
        });
        lastTxHash = handle.txHash;
        lastTxHash.replace('sync-tx:', '0x');
        lastTxReceipt = await handle.awaitReceipt();
    });

    it('should check api v0.2 account scope', async () => {
        const committedState = await provider.accountInfo(alice.address(), 'committed');
        const finalizedState = await provider.accountInfo(alice.address(), 'finalized');
        const fullState = await provider.accountFullInfo(alice.address());
        expect(fullState.committed, 'committed state differs').to.eql(committedState);
        expect(fullState.finalized, 'finalized state differs').to.eql(finalizedState);

        const txs = await provider.accountTxs(alice.accountId!, {
            from: lastTxHash,
            limit: 10,
            direction: 'older'
        });
        const expected = 4;
        expect(
            txs.list.length,
            `Endpoint returned incorrect number of transactions: ${txs.list.length}, expected ${expected}`
        ).to.eql(expected);
        expect(txs.list[0].txHash, 'Endpoint did not return first tx correctly').to.be.eql(lastTxHash);

        const accTxs = await provider.accountPendingTxs(alice.accountId!, {
            from: 1,
            limit: 10,
            direction: 'newer'
        });
        expect(accTxs).to.exist;
    });

    it('should check api v0.2 block scope', async () => {
        const lastCommittedBlock = await provider.blockByPosition('lastCommitted');
        expect(lastCommittedBlock).to.exist;

        const expectedBlocks = 3;
        const blocks = await provider.blockPagination({
            from: lastCommittedBlock.blockNumber,
            limit: 3,
            direction: 'older'
        });
        expect(
            blocks.list.length,
            `Endpoint returned incorrect number of blocks: ${blocks.list.length}, expected ${expectedBlocks}`
        ).to.eql(expectedBlocks);

        const expectedTxs = 1;
        const blockTxs = await provider.blockTransactions(lastTxReceipt.block!.blockNumber, {
            from: lastTxHash,
            limit: 10,
            direction: 'newer'
        });
        expect(
            blockTxs.list.length,
            `Endpoint returned incorrect number of transactions: ${blockTxs.list.length}, expected ${expectedTxs}`
        ).to.eql(expectedTxs);
    });

    it('should check api v0.2 config endpoint', async () => {
        const config = await provider.config();
        expect(config.network === 'localhost').to.be.true;
    });

    it('should check api v0.2 fee scope', async () => {
        const fee = await provider.getTransactionFee('Withdraw', alice.address(), 'ETH');
        expect(fee).to.exist;
        const batchFee = await provider.getBatchFullFee(
            [
                { txType: 'Transfer', address: alice.address() },
                { txType: 'Withdraw', address: alice.address() }
            ],
            'ETH'
        );
        expect(batchFee).to.exist;
    });

    it('should check api v0.2 network status endpoint', async () => {
        const networkStatus = await provider.networkStatus();
        expect(networkStatus).to.exist;
    });

    it('should check api v0.2 token scope', async () => {
        const tokens = await provider.tokenPagination({
            from: 0,
            limit: 2,
            direction: 'newer'
        });
        expect(tokens.list.length).to.be.eql(2);
        const firstToken = await provider.tokenByIdOrAddress('0x'.padEnd(42, '0'));
        const secondToken = await provider.tokenByIdOrAddress(1);
        expect(tokens.list[0]).to.be.eql(firstToken);
        expect(tokens.list[1]).to.be.eql(secondToken);
    });

    it('should check api v0.2 transaction scope', async () => {
        const apiReceipt = await provider.txStatus(lastTxHash);
        expect(apiReceipt!.rollupBlock).to.exist;

        const txData = await provider.txData(lastTxHash);
        expect(txData!.tx.op.type).to.eql('Transfer');

        const batch = await alice
            .batchBuilder()
            .addTransfer({ to: bob.address(), token: 'ETH', amount: alice.provider.tokenSet.parseToken('ETH', '1') })
            .addTransfer({ to: bob.address(), token: 'ETH', amount: alice.provider.tokenSet.parseToken('ETH', '1') })
            .build('ETH');
        const submitBatchResponse = await provider.submitTxsBatchNew(batch.txs, [batch.signature]);
        await provider.notifyAnyTransaction(submitBatchResponse.transactionHashes[0], 'COMMIT');
        const batchInfo = await provider.getBatch(submitBatchResponse.batchHash);
        expect(batchInfo.batchHash).to.eql(submitBatchResponse.batchHash);
    });
});
