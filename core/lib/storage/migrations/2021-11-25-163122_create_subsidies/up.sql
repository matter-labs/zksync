CREATE TABLE IF NOT EXISTS subsidies (
    id SERIAL PRIMARY KEY,
    tx_hash bytea NOT NULL,
    --- USD amounts are stored scaled by 10^6
    --- in other words, it is basically, fixed-point airthmetic with
    --- ~12 numbers before the decimal point and ~6 after
    --- It is safe, since in order to exceed ~4 * 10^18 limit of BIGINT it would take 10^12 dollars worth of subsidies
    usd_amount_scale6 BIGINT NOT NULL,
    full_cost_usd_scale6 BIGINT NOT NULL,
    token_id INT NOT NULL,
    token_amount NUMERIC NULL,
    full_cost_token NUMERIC NULL,
    subsidy_type VARCHAR NOT NULL
);
