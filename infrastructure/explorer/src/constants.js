export const PAGE_SIZE = 20;
export const TX_BATCH_SIZE = 50;

export const BLOCK_STORAGE_CONSTANT = 'BLOCK_STORAGE';
export const BLOCK_TRANSACTIONS_STORAGE_CONSTANT = 'BLOCK_TX_STORAGE';
export const TOKEN_STORAGE_CONSTANT = 'TOKEN_STORAGE';
export const TRANSACTION_STORAGE_CONSTANT = 'TRANSACTION_STORAGE';

export function getBlockStorageSlot(blockNumber) {
    return BLOCK_STORAGE_CONSTANT + blockNumber
}

export function getBlockTxStorageSlot(blockNumber) {
    return BLOCK_TRANSACTIONS_STORAGE_CONSTANT + blockNumber;
}

export function getTransactionStorageSlot(hash) {
    return TRANSACTION_STORAGE_CONSTANT + hash;   
}


