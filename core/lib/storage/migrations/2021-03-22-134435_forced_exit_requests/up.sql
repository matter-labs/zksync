CREATE TABLE forced_exit_requests (
    id BIGSERIAL PRIMARY KEY,
    target TEXT NOT NULL,
    tokens TEXT NOT NULL, -- comma-separated list of TokenIds
    price_in_wei NUMERIC NOT NULL,
    valid_until TIMESTAMP with time zone NOT NULL,
    created_at TIMESTAMP with time zone NOT NULL,
    fulfilled_by TEXT, -- comma-separated list of the hashes of ForcedExit transactions
    fulfilled_at TIMESTAMP with time zone
);
