CREATE TYPE eth_account_type AS ENUM ('Owned', 'CREATE2');

CREATE TABLE eth_account_types (
    account_id BIGINT PRIMARY KEY,
    account_type eth_account_type NOT NULL
);
