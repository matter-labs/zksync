CREATE TABLE withdrawals (
    id SERIAL PRIMARY KEY,
    account BYTEA NOT NULL,
    full_amount NUMERIC NOT NULL,
    remaining_amount NUMERIC NOT NULL,
    token_id INT NOT NULL,
    withdrawal_type TEXT NOT NULL,
    tx_hash BYTEA NOT NULL,
    tx_block BIGINT NOT NULL,
    tx_log_index BIGINT NOT NULL
);

CREATE TABLE finalized_withdrawals (
     id SERIAL PRIMARY KEY,
     pending_withdrawals_id SERIAL NOT NULL,
     amount NUMERIC NOT NULL,
     tx_hash BYTEA NOT NULL,
     tx_block BIGINT NOT NULL,
     tx_log_index BIGINT NOT NULL,
     CONSTRAINT fk_pending_withdrawals
        FOREIGN KEY (pending_withdrawals_id)
            REFERENCES withdrawals(id)
                ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS unique_withdrawals ON withdrawals(tx_hash, tx_log_index);
