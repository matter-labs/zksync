CREATE TABLE txs_count (
    id bigserial PRIMARY KEY,
    address bytea NOT NULL ,
    token int,
    count int not null,
    CONSTRAINT address_token_count UNIQUE (address, token)
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

        INSERT INTO txs_count (address, token, count) VALUES (NEW.address, NULL, 1)
        ON CONFLICT (address, token) DO UPDATE
            SET count = count + CASE WHEN EXISTS(SELECT 1 FROM tx_filters WHERE address = NEW.address AND tx_hash = NEW.tx_hash) THEN 0 ELSE 1 END

        INSERT INTO txs_count (address, token, count) VALUES (NEW.address, NEW.token, 1)
        ON CONFLICT (address, token) DO UPDATE
            SET count = count + CASE WHEN EXISTS(SELECT 1 FROM tx_filters WHERE address = NEW.address AND tx_hash = NEW.tx_hash AND token = NEW.token) THEN 0 ELSE 1 END

    END IF;
    RETURN NULL;
END;
$update_txs_count_for_address_tr$ LANGUAGE plpgsql;

CREATE TRIGGER update_txs_count_for_address_tr AFTER INSERT ON tx_filters
    FOR EACH ROW  EXECUTE FUNCTION update_txs_count_for_address();
