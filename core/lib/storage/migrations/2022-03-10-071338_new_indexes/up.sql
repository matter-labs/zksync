CREATE INDEX IF NOT EXISTS executed_priority_operations_tx_hash_idx ON  executed_priority_operations (tx_hash);
CREATE INDEX IF NOT EXISTS mint_nft_updates_account_id_block_number ON  mint_nft_updates (creator_account_id, block_number);
CREATE INDEX IF NOT EXISTS nft_creator_id_idx ON  nft (creator_account_id);
CREATE INDEX IF NOT EXISTS aggregate_operations_type_idx ON aggregate_operations (action_type);
