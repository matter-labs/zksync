-- This file should undo anything in `up.sql`
DROP TABLE IF EXISTS tree_restore_network CASCADE;
DROP TABLE IF EXISTS tree_restore_last_watched_eth_block CASCADE;
DROP TABLE IF EXISTS events_state CASCADE;
DROP TABLE IF EXISTS franklin_transactions CASCADE;