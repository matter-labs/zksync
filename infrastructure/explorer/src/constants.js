import store from './store';

export const PAGE_SIZE = 20;
export const TX_BATCH_SIZE = 50;

export const BLOCK_STORAGE_CONSTANT = 'BLOCK_STORAGE';
export const BLOCK_TRANSACTIONS_STORAGE_CONSTANT = 'BLOCK_TX_STORAGE';

export const blockchainExplorerTx = store.network === 'localhost' ? 'http://localhost:8000'
         : store.network === 'mainnet' ? `https://etherscan.io/tx`
         : `https://${store.network}.etherscan.io/tx`;

export const blockchainExplorerAddress = store.network === 'localhost' ? 'http://localhost:8000'
         : store.network === 'mainnet' ? `https://etherscan.io/address`
         : `https://${store.network}.etherscan.io/address`;

export const MAX_CACHED_BLOCKS = 20;
export const MAX_CACHED_BLOCKS_TRANSACTIONS = 5;
// Note that transactions are not saved to localStorage
// Thus its fine if we make the user store < 3MB in RAM
export const MAX_CACHED_TRANSACTIONS = 3000;
