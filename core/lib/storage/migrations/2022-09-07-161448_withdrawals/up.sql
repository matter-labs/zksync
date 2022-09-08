CREATE TABLE withdrawals (
    id SERIAL PRIMARY KEY,
    account BYTEA NOT NULL,
    amount BIGINT NOT NULL,
    token_id INT NOT NULL,
    withdrawal_type TEXT NOT NULL,
    pending_tx_hash BYTEA NOT NULL,
    pending_tx_block BIGINT NOT NULL,
    withdrawal_tx_hash BYTEA ,
    withdrawal_tx_block BIGINT
);