DROP TABLE txs_count;

DROP TRIGGER increase_txs_count_for_address_tr ON tx_filters;
DROP TRIGGER decrease_txs_count_for_address_tr ON tx_filters;


DROP FUNCTION increase_txs_count_for_address;
DROP FUNCTION decrease_txs_count_for_address;
