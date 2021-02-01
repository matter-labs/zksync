-- Your SQL goes here
ALTER TABLE eth_parameters RENAME COLUMN "commit_ops" TO "last_committed_block";
ALTER TABLE eth_parameters RENAME COLUMN "verify_ops" TO "last_verified_block";
ALTER TABLE eth_parameters RENAME COLUMN "withdraw_ops" TO "last_executed_block";

UPDATE eth_parameters
SET last_executed_block = (SELECT last_verified_block FROM eth_parameters WHERE id = true);
