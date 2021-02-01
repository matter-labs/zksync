CREATE TABLE forced_exit_requests (
    id BIGINT NOT NULL PRIMARY KEY,
    account_id BIGINT NOT NULL,
    token_id INTEGER NOT NULL,
    price_in_wei NUMERIC NOT NULL,
    valid_until TIMESTAMP with time zone NOT NULL
);
