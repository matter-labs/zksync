CREATE TABLE mempool_batches_signatures (
    batch_id bigserial PRIMARY KEY,
    eth_signature JSONB NOT NULL
);

CREATE OR REPLACE FUNCTION delete_signatures() RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM mempool_batches_signatures
        WHERE batch_id = ANY(
            SELECT DISTINCT batch_id FROM old_table
                WHERE batch_id > 0
        );
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER delete_signatures
    AFTER DELETE ON mempool_txs
    REFERENCING OLD TABLE AS old_table
    FOR EACH STATEMENT EXECUTE PROCEDURE delete_signatures();
