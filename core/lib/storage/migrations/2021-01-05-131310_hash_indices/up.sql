DROP INDEX IF EXISTS executed_transactions_from_account_index;
DROP INDEX IF EXISTS executed_transactions_to_account_index;

CREATE INDEX IF NOT EXISTS executed_transactions_from_account_idx ON "executed_transactions" USING hash (from_account);
CREATE INDEX IF NOT EXISTS executed_transactions_to_account_idx ON "executed_transactions" USING hash (to_account);
CREATE INDEX IF NOT EXISTS executed_transactions_primary_account_address_idx ON "executed_transactions" USING hash (primary_account_address);

CREATE INDEX IF NOT EXISTS account_creates_address_idx ON "account_creates" USING hash (address);
