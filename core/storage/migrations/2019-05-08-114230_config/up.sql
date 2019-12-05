CREATE TABLE server_config(    
    -- enforce single record
    id              bool PRIMARY KEY NOT NULL DEFAULT true,
    CONSTRAINT      single_server_config CHECK (id),

    contract_addr   text,
    gov_contract_addr text
);
