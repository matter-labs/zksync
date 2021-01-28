-- This file should undo anything in `up.sql`
ALTER TABLE eth_parameters RENAME COLUMN "last_committed_block" TO "commit_ops";
ALTER TABLE eth_parameters RENAME COLUMN "last_verified_block" TO "verify_ops";
ALTER TABLE eth_parameters RENAME COLUMN "last_executed_block" TO "withdraw_ops";
