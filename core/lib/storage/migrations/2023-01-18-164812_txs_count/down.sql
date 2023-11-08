DROP TRIGGER IF EXISTS increase_txs_count_for_address_tr ON tx_filters;
DROP TRIGGER IF EXISTS decrease_txs_count_for_address_tr ON tx_filters;

DROP FUNCTION IF EXISTS increase_txs_count_for_address;
DROP FUNCTION IF EXISTS decrease_txs_count_for_address;

DROP TABLE txs_count;
