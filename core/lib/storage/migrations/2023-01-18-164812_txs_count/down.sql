DROP TABLE txs_count;

DROP TRIGGER update_txs_count_for_address_tr ON tx_filters;
DROP FUNCTION update_txs_count_for_address;