-- This file should undo anything in `up.sql`
DROP TABLE IF EXISTS node_restore_last_watched_eth_block CASCADE;
DROP TABLE IF EXISTS events_state CASCADE;
DROP TABLE IF EXISTS franklin_ops CASCADE;
DROP TABLE IF EXISTS storage_state_update CASCADE;
