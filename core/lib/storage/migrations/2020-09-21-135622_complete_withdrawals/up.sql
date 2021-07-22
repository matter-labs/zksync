CREATE TABLE pending_withdrawals (
    id BIGINT NOT NULL,
    withdrawal_hash bytea NOT NULL,
    PRIMARY KEY (id)
);
CREATE TABLE complete_withdrawals_transactions (
    tx_hash bytea NOT NULL,
    pending_withdrawals_queue_start_index BIGINT NOT NULL,
    pending_withdrawals_queue_end_index BIGINT NOT NULL,
    PRIMARY KEY (tx_hash)
);
