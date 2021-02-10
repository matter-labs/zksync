import LRU from 'lru-cache';
import { MAX_CACHED_BLOCKS, MAX_CACHED_BLOCKS_TRANSACTIONS, MAX_CACHED_TRANSACTIONS } from './constants';
import { getTxFee, getFromAddressOfTx, getTxToAddress, getTxAmount, getTxToken } from './blockUtils';

class Cacher {
    initLRUCaches() {
        this.blocksCache = new LRU(MAX_CACHED_BLOCKS);
        this.blocksTxsCache = new LRU(MAX_CACHED_BLOCKS_TRANSACTIONS);
        this.txCache = new LRU(MAX_CACHED_TRANSACTIONS);
    }

    getCachedBlock(blockNumber) {
        return this.blocksCache.get(blockNumber);
    }

    getCachedBlockTransactions(blockNumber) {
        return this.blocksTxsCache.get(blockNumber);
    }

    getCachedTransaction(hash) {
        return this.txCache.get(hash);
    }

    cacheBlock(blockNumber, block) {
        this.blocksCache.set(blockNumber, block);
    }

    cacheBlockTransactions(blockNumber, txs) {
        this.blocksTxsCache.set(blockNumber, txs);
    }

    // Technically, we could also cache transactions also from an account.
    // But it introduces too many complications.
    cacheTransactionsFromBlock(txs, client) {
        // We do not await for the Promises, because
        //
        // a) JS is single-threaded, no data-races possible
        // b) By the time we will want to reuse these transactions there is a
        // 99% change they will be set, or, if not, it does not ruin anything.
        txs.forEach(async (tx) => {
            const op = tx.op;
            this.cacheTransaction(tx.tx_hash, {
                tx_type: op.type,
                from: getFromAddressOfTx(tx),
                to: getTxToAddress(tx),
                token: getTxToken(tx),
                amount: await getTxAmount(tx, client),
                fee: getTxFee(tx),
                block_number: tx.block_number,
                nonce: op.nonce,
                created_at: tx.created_at,
                fail_reason: tx.fail_reason,
                tx: op
            });
        });
    }

    cacheTransaction(hash, tx) {
        this.txCache.set(hash, tx);
    }

    constructor(client) {
        this.initLRUCaches();
    }
}

export default Cacher;
