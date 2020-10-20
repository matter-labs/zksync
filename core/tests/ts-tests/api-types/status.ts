export type Interface = {
    next_block_at_max: number | null;
    last_committed: number;
    last_verified: number;
    total_transactions: number;
    outstanding_txs: number;
};
