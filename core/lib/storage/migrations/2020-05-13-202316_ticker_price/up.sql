CREATE TABLE ticker_price (
    token_id INTEGER NOT NULL REFERENCES tokens(id) ON UPDATE CASCADE,
    usd_price NUMERIC NOT NULL,
    last_updated TIMESTAMP with time zone NOT NULL,
    PRIMARY KEY (token_id)
)
