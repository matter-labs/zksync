-- To prevent mempool from filling blocks with reverted priority operations
-- out of order, we keep a track of them.
ALTER TABLE mempool_txs ADD COLUMN next_priority_op_serial_id BIGINT DEFAULT NULL;
