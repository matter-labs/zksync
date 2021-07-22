CREATE TABLE nft_factory
(
    creator_id INTEGER PRIMARY KEY,
    factory_address TEXT NOT NULL,
    creator_address TEXT NOT NULL,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT now()
);
