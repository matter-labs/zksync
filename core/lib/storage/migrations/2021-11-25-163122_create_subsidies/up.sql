CREATE TABLE subsidies (
    serial_id BIGINT NOT NULL,
    tx_hash bytea NOT NULL,
    usd_amount NUMERIC NOT NULL,
    full_cost_usd NUMERIC NOT NULL,
    token_id INT NOT NULL,
    token_amount NUMERIC NULL,
    full_cost_token NUMERIC NULL,
    subsidy_type VARCHAR NOT NULL
);
