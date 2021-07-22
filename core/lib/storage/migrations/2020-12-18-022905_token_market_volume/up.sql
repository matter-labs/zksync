CREATE TABLE ticker_market_volume (
    token_id INTEGER NOT NULL REFERENCES tokens(id) ON UPDATE CASCADE,
    market_volume NUMERIC NOT NULL,
    last_updated TIMESTAMP with time zone NOT NULL,
    PRIMARY KEY (token_id)
)
