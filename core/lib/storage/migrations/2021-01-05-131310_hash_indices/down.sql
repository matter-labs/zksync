CREATE INDEX IF NOT EXISTS executed_transactions_from_account_index ON "executed_transactions" USING btree (from_account);
CREATE INDEX IF NOT EXISTS executed_transactions_to_account_index ON "executed_transactions" USING btree (to_account);

DROP INDEX IF EXISTS executed_transactions_from_account_idx;
DROP INDEX IF EXISTS executed_transactions_to_account_idx;
DROP INDEX IF EXISTS executed_transactions_primary_account_address_idx;

DROP INDEX IF EXISTS account_creates_address_idx;
