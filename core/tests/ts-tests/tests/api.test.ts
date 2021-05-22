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
        tester = await Tester.init('localhost', 'HTTP');
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
    let provider: RestProvider;
    let lastTxHash: string;
    let lastTxReceipt: types.ApiTxReceipt;

    before('create tester and test wallets', async () => {
        tester = await Tester.init('localhost', 'HTTP');
        alice = await tester.fundedWallet('1.0');
        let bob = await tester.emptyWallet();
        provider = await getDefaultRestProvider('localhost');
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

        const handle = await alice.syncTransfer({
            to: bob.address(),
            token: 'ETH',
            amount: alice.provider.tokenSet.parseToken('ETH', '1')
        });
        lastTxHash = handle.txHash.replace('sync-tx:', '0x');
        lastTxReceipt = await provider.notifyAnyTransaction(lastTxHash, 'COMMIT');
    });

    it('should check api v0.2 account scope', async () => {
        const accountCommittedInfo = await provider.accountInfo(alice.address(), 'committed');
        expect(accountCommittedInfo != null, 'Account does not have committed state').to.be.true;

        await provider.accountInfo(alice.address(), 'finalized');

        const txs = await provider.accountTxs(alice.accountId!, {
            from: lastTxHash,
            limit: 10,
            direction: 'older'
        });
        expect(txs.list.length > 1, 'Endpoint returned not all txs').to.be.true;
        expect(txs.list[0].txHash, 'Endpoint returned wrong first tx').to.be.eql(lastTxHash);

        await provider.accountPendingTxs(alice.accountId!, {
            from: 1,
            limit: 10,
            direction: 'newer'
        });
    });

    it('should check api v0.2 block scope', async () => {
        const blocks = await provider.blockPagination({
            from: lastTxReceipt.rollupBlock!,
            limit: 10,
            direction: 'older'
        });
        expect(blocks.list.length > 0, 'Endpoint returned not all blocks').to.be.true;

        const lastCommittedBlock = await provider.blockByPosition('lastCommitted');
        const lastCommittedBlockByNumber = await provider.blockByPosition(lastTxReceipt.rollupBlock!);
        expect(lastCommittedBlock).to.be.eql(lastCommittedBlockByNumber);

        const blockTxs = await provider.blockTransactions('lastCommitted', {
            from: lastTxHash,
            limit: 10,
            direction: 'newer'
        });
        expect(blockTxs.list[0].txHash).to.be.eql(lastTxHash);
    });

    it('should check api v0.2 config endpoint', async () => {
        const config = await provider.config();
        expect(config.network === 'localhost').to.be.true;
    });

    it('should check api v0.2 fee scope', async () => {
        await provider.getTransactionFee('Withdraw', alice.address(), 'ETH');
        await provider.getBatchFullFee(
            [
                { txType: 'Transfer', address: alice.address() },
                { txType: 'Withdraw', address: alice.address() }
            ],
            'ETH'
        );
    });

    it('should check api v0.2 network status endpoint', async () => {
        const networkStatus = await provider.networkStatus();
        expect(networkStatus.lastCommitted).to.be.eql(lastTxReceipt.rollupBlock);
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

    // it('should check api v0.2 transaction scope', async () => {

    // });
});
