DO
$$
    DECLARE
        a1 BYTEA;
        a2 BYTEA;
        rows BIGINT = 10000;
    BEGIN
        LOOP
            SELECT address INTO a2
            FROM
                (
                    SELECT DISTINCT address
                    FROM tx_filters
                    WHERE ( a1 IS NULL OR address > a1 )
                    ORDER BY address
                    LIMIT rows
                ) AS a
            ORDER BY address DESC
            LIMIT 1;

            IF NOT found THEN EXIT; END IF;

            INSERT INTO txs_count (address, token, count)
            SELECT address,token, COUNT(DISTINCT tx_hash)
            FROM tx_filters
            WHERE ( a1 IS NULL OR address > a1 ) AND address <= a2
                GROUP BY (address, token)
            ON CONFLICT( address, token) DO UPDATE SET count = EXCLUDED.count;

            INSERT INTO txs_count (address, token, count)
            SELECT address,NULL, COUNT(DISTINCT tx_hash)
            FROM tx_filters
            WHERE ( a1 IS NULL OR address > a1 ) AND address <= a2
                GROUP BY (address)
            ON CONFLICT( address, token) DO UPDATE SET count = EXCLUDED.count;
            raise info 'from %: to %', a1, a2;
            COMMIT;
            a1 = a2;
        END LOOP;
    END;
$$