ALTER TABLE executed_priority_operations
    ADD eth_block_index bigint;
ALTER TABLE executed_priority_operations
    ADD tx_hash bytea NOT NULL DEFAULT ''::bytea;
-- Calculates hashes for existing priority ops.
UPDATE executed_priority_operations
    SET tx_hash = sha256(eth_hash::bytea || int8send(eth_block)::bytea || int8send(0)::bytea);

CREATE TABLE txs_batches_hashes (
    batch_id BIGSERIAL PRIMARY KEY,
    batch_hash bytea NOT NULL
);
-- Calculates hashes for existing batches. It gets transactions from 
-- executed_transactions table for every batch_id that exists in db and calculates sha256
-- of their tx_hashes concat.
DO $$
DECLARE
max_batch_id bigint;
agg_hash bytea;
rec record;
BEGIN
    SELECT MAX(batch_id) FROM executed_transactions 
	    WHERE batch_id IS NOT NULL 
  	        INTO max_batch_id;
    IF max_batch_id IS NOT NULL THEN
        FOR i IN 0..max_batch_id
        LOOP
            agg_hash = '';
            FOR rec in SELECT tx_hash FROM executed_transactions
                        WHERE batch_id = i
                        ORDER BY created_at
            LOOP
                agg_hash = agg_hash || rec.tx_hash;
            END LOOP;
            IF length(agg_hash) != 0 THEN
                INSERT INTO txs_batches_hashes (batch_id, batch_hash)
                VALUES (i, sha256(agg_hash));
            END IF;
        END LOOP;
    END IF;
END;
$$;
