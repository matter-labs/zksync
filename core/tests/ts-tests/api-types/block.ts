export type Interface = {
    block_number: number;
    new_state_root: string;
    block_size: number;
    commit_tx_hash: string | null;
    verify_tx_hash: string | null;
    committed_at: string;
    verified_at: string | null;
};
