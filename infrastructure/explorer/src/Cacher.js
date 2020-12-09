import {
    BLOCK_STORAGE_CONSTANT,
    BLOCK_TRANSACTIONS_STORAGE_CONSTANT,
    MAX_CACHED_BLOCKS,
    MAX_CACHED_BLOCKS_TRANSACTIONS,
    MAX_CACHED_TRANSACTIONS
} from './constants';

import  {
    getTxFee,
    getTxFromAddress,
    getTxToAddress,
    getTxAmount,
    getTxToken
} from './blockUtils';


class Cacher {

    loadCachedBlocks() {
        const stored = localStorage.getItem(BLOCK_STORAGE_CONSTANT);
        this.cachedBlocks = stored ? JSON.parse(stored): {};
    }

    loadCachedBlocksTransactions() {
        const stored = localStorage.getItem(BLOCK_TRANSACTIONS_STORAGE_CONSTANT);
        this.cachedBlocksTransactions = stored ? JSON.parse(stored) : {};
    }

    getCachedBlock(blockNumber) {
        return this.cachedBlocks[blockNumber];
    }

    getCachedBlockTransactions(blockNumber) {
        return this.cachedBlocksTransactions[blockNumber];
    }

    getCachedTransaction(hash) {
        return this.cachedTransactions[hash];
    }

    freeSpaceForCache(obj, maxLeft) {
        const objKeys = Object.keys(obj);
        const objSize = objKeys.length;

        if(objSize < maxLeft) {
            return;
        }

        let toDelete = objSize - maxLeft;
        
        // I delete the first toDelete elements, because:
        //
        // a) It does not really matter what elements to delete
        // b) The first elements are usually the smallest one, thus 
        // we delete the most irrelevant blocks 
        for(let i = 0; i < toDelete; i++) {
            const key = objKeys[i];
            delete obj[key];
        }
    }

    cacheBlock(blockNumber, block) {
        this.freeSpaceForCache(
            this.cachedBlocks,
            MAX_CACHED_BLOCKS-1
        );
        this.cachedBlocks[blockNumber] = block;
    }

    cacheBlockTransactions(blockNumber, txs) {
        this.freeSpaceForCache(
            this.cachedBlocksTransactions,
            MAX_CACHED_BLOCKS_TRANSACTIONS-1
        );
        this.cachedBlocksTransactions[blockNumber] = txs;
    }

    // Technically, we could also cache transactions also from an account.
    // But it introduces too many complications.
    cacheTransactionsFromBlock(txs, client) {
        const numbrerOfTxToCache = Math.min(txs.length, MAX_CACHED_TRANSACTIONS);
        this.freeSpaceForCache(
            this.cachedTransactions,
            // Leaving just enough room for numbrerOfTxToCache
            // transactions
            MAX_CACHED_TRANSACTIONS - numbrerOfTxToCache
        );

        txs.slice(0, numbrerOfTxToCache).forEach((tx) => {
            const op = tx.op;
            this.cachedTransactions[tx.tx_hash] = {
                tx_type: op.type,
                from: getTxFromAddress(op),
                to: getTxToAddress(op),
                token: getTxToken(op),
                amount: getTxAmount(op, client),
                fee: getTxFee(op),
                block_number: tx.block_number,
                nonce: op.nonce,
                created_at: tx.created_at,
                fail_reason: tx.fail_reason,
                tx: op
            };
        });
    }

    cacheTransaction(hash, tx) {
        this.freeSpaceForCache(
            this.cachedTransactions, 
            MAX_CACHED_TRANSACTIONS - 1
        );
        this.cachedTransactions[hash] = tx; 
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
        localStorage.set(
            BLOCK_STORAGE_CONSTANT,
            JSON.stringify(this.cachedBlocks)
        );
        localStorage.set(
            BLOCK_TRANSACTIONS_STORAGE_CONSTANT,
            JSON.stringify(this.cachedBlocksTransactions)
        );
    }

    constructor(client) {
        this.loadCachedBlocks();
        this.loadCachedBlocksTransactions();
        this.cachedTransactions = {};

        
        Object.values(this.cachedBlocksTransactions).forEach((txs) => {
            this.cacheTransactionsFromBlock(txs, client);
        });
    }

}

export default Cacher; 
