CREATE TABLE forced_exit_requests (
    id BIGSERIAL PRIMARY KEY,
    target TEXT NOT NULL,
    tokens TEXT NOT NULL,
    price_in_wei NUMERIC NOT NULL,
    valid_until TIMESTAMP with time zone NOT NULL,
    fulfilled_at TIMESTAMP with time zone
);
