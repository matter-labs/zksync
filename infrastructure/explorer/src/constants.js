import store from './store';
import { getBlockchainExplorerTx, getBlockchainExplorerAddress } from './utils';

export const PAGE_SIZE = 20;
export const TX_BATCH_SIZE = 50;

export const blockchainExplorerTx = getBlockchainExplorerTx(store.network);

export const blockchainExplorerAddress = getBlockchainExplorerAddress(store.network);

export const MAX_CACHED_BLOCKS = 20;
export const MAX_CACHED_BLOCKS_TRANSACTIONS = 5;
// Note that transactions are not saved to localStorage
// Thus its fine if we make the user store < 3MB in RAM
export const MAX_CACHED_TRANSACTIONS = 3000;
