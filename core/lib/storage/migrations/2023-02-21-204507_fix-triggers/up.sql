DROP TRIGGER IF EXISTS increase_txs_count_for_address_tr ON tx_filters;
DROP TRIGGER IF EXISTS decrease_txs_count_for_address_tr ON tx_filters;

CREATE OR REPLACE FUNCTION decrease_txs_count_for_address() RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'DELETE') THEN
        -- Postgresql doesn't support unique indexes for nullable fields, so we have to use
        -- artificial token which means no token
        UPDATE txs_count SET count = txs_count.count -
                                     CASE WHEN (SELECT count(*) = 1  FROM tx_filters WHERE address = OLD.address AND tx_hash = OLD.tx_hash) THEN 1 ELSE 0 END
        WHERE address=OLD.address AND token = -1;

        UPDATE txs_count SET count = txs_count.count -
                                     CASE WHEN (SELECT count(*) = 1 FROM tx_filters WHERE address = OLD.address AND tx_hash = OLD.tx_hash AND token = OLD.token) THEN 1 ELSE 0 END
        WHERE address=OLD.address AND token=OLD.token;

    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION increase_txs_count_for_address() RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        INSERT INTO txs_count (address, token, count) VALUES (NEW.address, -1       , 0) ON CONFLICT (address, token) DO NOTHING;
        INSERT INTO txs_count (address, token, count) VALUES (NEW.address, NEW.token, 0) ON CONFLICT (address, token) DO NOTHING;

        UPDATE txs_count SET count = txs_count.count +
                                     CASE WHEN EXISTS(SELECT 1 FROM tx_filters WHERE address = NEW.address AND tx_hash = NEW.tx_hash FOR UPDATE) THEN 0 ELSE 1 END
        WHERE address = NEW.address AND token = -1;

        UPDATE txs_count SET count = txs_count.count +
                                     CASE WHEN EXISTS(SELECT 1 FROM tx_filters WHERE address = NEW.address AND tx_hash = NEW.tx_hash AND token = NEW.token) THEN 0 ELSE 1 END
        WHERE address = NEW.address AND token = NEW.token;

    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER increase_txs_count_for_address_tr BEFORE INSERT ON tx_filters
    FOR EACH ROW  EXECUTE FUNCTION increase_txs_count_for_address();

CREATE TRIGGER decrease_txs_count_for_address_tr BEFORE DELETE ON tx_filters
    FOR EACH ROW  EXECUTE FUNCTION decrease_txs_count_for_address();
