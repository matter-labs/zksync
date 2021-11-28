CREATE TABLE subsidies (
    id SERIAL PRIMARY KEY,
    tx_hash bytea NOT NULL,
    usd_amount BIGINT NOT NULL,
    full_cost_usd BIGINT NOT NULL,
    token_id INT NOT NULL,
    token_amount NUMERIC NULL,
    full_cost_token NUMERIC NULL,
    subsidy_type VARCHAR NOT NULL
);
