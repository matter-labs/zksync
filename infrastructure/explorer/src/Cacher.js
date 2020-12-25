import LRU from 'lru-cache';
import {
    BLOCK_STORAGE_SLOT,
    BLOCK_TRANSACTIONS_STORAGE_SLOT,
    MAX_CACHED_BLOCKS,
    MAX_CACHED_BLOCKS_TRANSACTIONS,
    MAX_CACHED_TRANSACTIONS,
    CACHE_VERSION_SLOT,
    CACHE_VERSION
} from './constants';
import { getTxFee, getFromAddressOfTx, getTxToAddress, getTxAmount, getTxToken } from './blockUtils';

class Cacher {
    checkCacheVersion() {
        const version = localStorage.getItem(CACHE_VERSION_SLOT);
        if (version !== CACHE_VERSION) {
            localStorage.removeItem(BLOCK_STORAGE_SLOT);
            localStorage.removeItem(BLOCK_TRANSACTIONS_STORAGE_SLOT);

            localStorage.setItem(CACHE_VERSION_SLOT, CACHE_VERSION);
        }
    }

    initLRUCaches() {
        this.blocksCache = new LRU(MAX_CACHED_BLOCKS);
        this.blocksTxsCache = new LRU(MAX_CACHED_BLOCKS_TRANSACTIONS);
        this.txCache = new LRU(MAX_CACHED_TRANSACTIONS);
    }

    load(cache, slot) {
        const stored = localStorage.getItem(slot);
        if (!stored) {
            return;
        }

        try {
            cache.load(JSON.parse(stored));
        } catch {
            localStorage.removeItem(slot);
        }
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
        txs.forEach(async tx => {
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

    saveCacheToLocalStorage() {
        // We don't store transactions here, because:
        // a) A lot of them are already stored with block transactions
        // b) It is unlikely that if a person wants to open a transaction
        // first it will be verified, i.e. suitable for caching.
        // c) We can not simply store all the transactions we want.
        // localStorage has it's own limitation.
        //
        // Although to reach unlimited memory we could use
        // https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API
        // But I believe it is an overkill
        const blocksCacheDump = this.blocksCache.dump();
        const blocksTxsCacheDump = this.blocksTxsCache.dump();
        localStorage.setItem(BLOCK_STORAGE_SLOT, JSON.stringify(blocksCacheDump));
        localStorage.setItem(BLOCK_TRANSACTIONS_STORAGE_SLOT, JSON.stringify(blocksTxsCacheDump));
    }

    constructor(client) {
        this.checkCacheVersion();
        this.initLRUCaches();
        this.load(this.blocksCache, BLOCK_STORAGE_SLOT);
        this.load(this.blocksTxsCache, BLOCK_TRANSACTIONS_STORAGE_SLOT);

        this.blocksTxsCache.values().forEach(txs => {
            this.cacheTransactionsFromBlock(txs, client);
        });
    }
}

export default Cacher;
