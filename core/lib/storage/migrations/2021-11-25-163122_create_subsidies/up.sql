CREATE TABLE subsidies (
    id SERIAL PRIMARY KEY,
    tx_hash bytea NOT NULL,
    --- USD amounts are stored scaled by 10^8
    --- in other words, it is basically, fixed-point airthmetic with
    --- ~10 numbers before the decimal point and ~8 after
    --- It is safe, since in order to exceed ~4 * 10^18 limit of BIGINT it would take 10^10 dollars worth of subsidies
    usd_amount_scale8 BIGINT NOT NULL,
    full_cost_usd_scale8 BIGINT NOT NULL,
    token_id INT NOT NULL,
    token_amount NUMERIC NULL,
    full_cost_token NUMERIC NULL,
    subsidy_type VARCHAR NOT NULL
);
