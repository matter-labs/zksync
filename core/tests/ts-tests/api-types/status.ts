export type Interface = {
    next_block_at_max: number | null;
    last_committed: number;
    last_verified: number;
    total_transactions: number;
    outstanding_txs: number;
    mempool_size: number;
    core_status: null | CoreStatus
};

export type CoreStatus = {
    main_database_status: boolean;
    replica_database_status: boolean;
    eth_status: boolean;
}
