ALTER TABLE executed_priority_operations
    ADD eth_block_index bigint;
ALTER TABLE executed_priority_operations
    ADD tx_hash bytea NOT NULL DEFAULT ''::bytea;
UPDATE executed_priority_operations
    SET tx_hash = sha256(eth_hash::bytea || int8send(eth_block)::bytea || int8send(0)::bytea);

CREATE TABLE txs_batches_hashes (
    batch_id BIGSERIAL PRIMARY KEY,
    batch_hash bytea NOT NULL
);

DO $$
DECLARE
max_batch_id bigint;
hashes bytea[] = '{}';
agg_hashes bytea;
rec record;
BEGIN
    SELECT MAX(batch_id) FROM executed_transactions 
	    WHERE batch_id IS NOT NULL 
  	        INTO max_batch_id;
    FOR i IN 0..max_batch_id
    LOOP
        agg_hashes = '';
        FOR rec in SELECT tx_hash FROM executed_transactions 
                    WHERE batch_id = i
                    ORDER BY created_at
        LOOP
            agg_hashes = agg_hashes || rec.tx_hash;
        END LOOP;
        IF length(agg_hashes) != 0 THEN
            INSERT INTO txs_batches_hashes (batch_id, batch_hash)
            VALUES (i, sha256(agg_hashes));
        END IF;
    END LOOP;
END;
$$;
