CREATE TABLE txs_count (
    address BYTEA NOT NULL ,
    token INT NOT NULL,
    count BIGINT NOT NULL,
    CONSTRAINT address_token_count PRIMARY KEY (address, token)
);

CREATE OR REPLACE FUNCTION update_txs_count_for_address() RETURNS TRIGGER AS $update_txs_count_for_address_tr$
DECLARE
    _dummy bigint;
BEGIN
    IF (TG_OP = 'INSERT') THEN

        /*
        REQUIRED IF DEADLOCK WILL APPEAR
        SELECT count(*) INTO _dummy
        FROM txs_count
        WHERE address = NEW.address AND ( token IS NULL OR token = NEW.token )
        FOR UPDATE;
        */

        -- Postgresql doesn't support unique indexes for nullable fields, so we have to use
        -- artificial token which means no token
        INSERT INTO txs_count (address, token, count) VALUES (NEW.address, -1, 1)
        ON CONFLICT (address, token) DO UPDATE
            SET count = txs_count.count + CASE WHEN EXISTS(SELECT 1 FROM tx_filters WHERE address = NEW.address AND tx_hash = NEW.tx_hash) THEN 0 ELSE 1 END;

        INSERT INTO txs_count (address, token, count) VALUES (NEW.address, NEW.token, 1)
        ON CONFLICT (address, token) DO UPDATE
            SET count = txs_count.count + CASE WHEN EXISTS(SELECT 1 FROM tx_filters WHERE address = NEW.address AND tx_hash = NEW.tx_hash AND token = NEW.token) THEN 0 ELSE 1 END;

    END IF;
    RETURN NULL;
END;
$update_txs_count_for_address_tr$ LANGUAGE plpgsql;

CREATE TRIGGER update_txs_count_for_address_tr BEFORE INSERT ON tx_filters
    FOR EACH ROW  EXECUTE FUNCTION update_txs_count_for_address();
