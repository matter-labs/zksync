-- Unfortunatley there is no easy way to remove a type from enum, so we'll have to just 
-- drop the old one and cast the types

ALTER TYPE eth_account_type ADD VALUE 'No2FA';

CREATE TYPE eth_accont_type_old AS ENUM ('Owned', 'CREATE2');

UPDATE eth_account_types SET account_type='Owned' WHERE account_type='No2FA';

ALTER TABLE eth_account_types
    ALTER COLUMN account_type TYPE eth_accont_type_old
        USING (account_type::text::eth_accont_type_old);

DROP TYPE eth_account_type;

ALTER TYPE eth_accont_type_old RENAME TO eth_account_type;
