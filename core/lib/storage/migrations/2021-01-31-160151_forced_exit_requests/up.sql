CREATE TABLE forced_exit_requests (
    id BIGSERIAL PRIMARY KEY,
    target TEXT NOT NULL,
    tokens TEXT NOT NULL,
    price_in_wei NUMERIC NOT NULL,
    valid_until TIMESTAMP with time zone NOT NULL,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT NOW(),
    fulfilled_at TIMESTAMP with time zone
)
