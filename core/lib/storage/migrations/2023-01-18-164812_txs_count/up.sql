CREATE TABLE txs_count (
    id bigserial PRIMARY KEY,
    address bytea,
    token int,
    count int not null,
    last_seq_no bigint,
    CONSTRAINT address_token_count UNIQUE (address, token)
);


INSERT INTO txs_count (address, count, last_seq_no) SELECT NULL, SUM(countTxs), max(sequence_number) FROM
    (
        select count(*) countTxs, MAX(sequence_number) sequence_number from executed_transactions where success = true
        UNION ALL
        select count(*) countTxs, MAX(sequence_number) sequence_number from executed_priority_operations
    ) as cTsncTsn ;

INSERT INTO txs_count (address, token, count, last_seq_no) SELECT address,token, count(distinct tx_hash), max(sequence_number)! from tx_filters group by (address, token);


CREATE OR REPLACE FUNCTION update_all_txs_count() RETURNS TRIGGER AS $update_all_txs_count_1$
    BEGIN
--         IF (!NEW.success) THEN
--             RETURN NULL;
--         END IF;

        IF (TG_OP = 'DELETE') THEN
            UPDATE txs_count SET count = count - 1 WHERE address is null;
        ELSIF (TG_OP = 'INSERT') THEN
            UPDATE txs_count SET count = count + 1, last_seq_no = NEW.sequence_number  WHERE address is null;
        END IF;
        RETURN NULL;
    END;
$update_all_txs_count_1$ LANGUAGE plpgsql;


CREATE TRIGGER update_executed_txs_count AFTER INSERT OR DELETE ON executed_transactions
    FOR EACH ROW  EXECUTE FUNCTION update_all_txs_count();

CREATE TRIGGER update_priority_op_txs_count AFTER INSERT ON executed_priority_operations
    FOR EACH ROW  EXECUTE FUNCTION update_all_txs_count();

CREATE OR REPLACE FUNCTION update_txs_count_for_address() RETURNS TRIGGER AS $update_all_txs_count_2$
DECLARE
    last_seq_no INT := (SELECT last_seq_no from txs_count WHERE address = NEW.address and token = NEW.token);
    tx_count INT := (SELECT count(*) FROM tx_filters WHERE address = NEW.address AND token = NEW.token AND sequence_number > last_seq_no);
BEGIN
    IF (TG_OP = 'INSERT') THEN
        INSERT INTO txs_count (address, token, count, last_seq_no) VALUES (NEW.address, NEW.token, tx_count, NEW.sequence_number)
        ON CONFLICT (address, token) DO UPDATE
        SET count = txs_count.count + tx_count,
            last_seq_no = NEW.sequence_number;
    END IF;
    RETURN NULL;
END;
$update_all_txs_count_2$ LANGUAGE plpgsql;

CREATE TRIGGER update_txs_count_for_address_tr AFTER INSERT ON tx_filters
    FOR EACH ROW  EXECUTE FUNCTION update_txs_count_for_address();

CREATE INDEX IF NOT EXISTS txs_count_index ON txs_count(address, token);
