import store from './store';
import { getBlockchainExplorerTx, getBlockchainExplorerAddress } from './utils';

export const PAGE_SIZE = 20;
export const TX_BATCH_SIZE = 50;

export const BLOCK_STORAGE_SLOT = 'BLOCK_STORAGE';
export const BLOCK_TRANSACTIONS_STORAGE_SLOT = 'BLOCK_TX_STORAGE';
export const CACHE_VERSION_SLOT = 'CACHE_VERSION';
// In the future our cache utilities / api responses might change
// When it happens, we must also change the CACHE_VERSION.
//
// If the CACHE_VERSION on a client's computer is incorrect,
// it wiil be reset.
export const CACHE_VERSION = 1;

export const blockchainExplorerTx = getBlockchainExplorerTx(store.network);

export const blockchainExplorerAddress = getBlockchainExplorerAddress(store.network);

export const MAX_CACHED_BLOCKS = 20;
export const MAX_CACHED_BLOCKS_TRANSACTIONS = 5;
// Note that transactions are not saved to localStorage
// Thus its fine if we make the user store < 3MB in RAM
export const MAX_CACHED_TRANSACTIONS = 3000;
