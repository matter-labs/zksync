import { 
    getBlockStorageSlot,
    getBlockTxStorageSlot, 
    getTransactionStorageSlot
} from './constants';

export function getCachedBlock(blockNumber) {
    const storageSlot = getBlockStorageSlot(blockNumber);
    const cachedValue = localStorage.getItem(storageSlot);
    
    return JSON.parse(cachedValue);
}

export function getCachedBlockTransactions(blockNumber) {
    const storageSlot = getBlockTxStorageSlot(blockNumber);
    const cachedValue = localStorage.getItem(storageSlot);
    if(!cachedValue) {
        return undefined;
    }

    return JSON.parse(cachedValue);
}

// export function getCachedTransaction(hash) {
//     const storageSlot = getTransactionStorageSlot(hash);
//     const cachedValue = localStorage.getItem(storageSlot);

//     if(!cachedValue) {
//         return undefined;
//     }

//     return JSON.parse(cachedValue);
// }

export function cacheBlock(block) {
    const blockNumber = block.block_number;
    const storageSlot = getBlockStorageSlot(blockNumber);

    localStorage.setItem(storageSlot, JSON.stringify(block));
}

export function cacheBlockTransactions(blockNumber, transactions) {
    const storageSlot = getBlockTxStorageSlot(blockNumber);

    localStorage.setItem(storageSlot, JSON.stringify(transactions));
}

// export function cacheTransaction(tx) {
//     const hash = tx.tx_hash;
//     const storageSlot = getTransactionStorageSlot(hash);

//     localStorage.setItem(storageSlot, JSON.stringify(tx));
// }
