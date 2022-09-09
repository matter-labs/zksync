CREATE TABLE withdrawals (
    id SERIAL PRIMARY KEY,
    account BYTEA NOT NULL,
    amount NUMERIC NOT NULL,
    token_id INT NOT NULL,
    withdrawal_type TEXT NOT NULL,
    pending_tx_hash BYTEA NOT NULL,
    pending_tx_block BIGINT NOT NULL,
    pending_tx_log_index BIGINT NOT NULL,
    withdrawal_tx_hash BYTEA ,
    withdrawal_tx_block BIGINT,
    withdrawal_tx_log_index BIGINT
);
CREATE UNIQUE INDEX IF NOT EXISTS unique_withdrawals ON withdrawals(pending_tx_hash, pending_tx_log_index);
