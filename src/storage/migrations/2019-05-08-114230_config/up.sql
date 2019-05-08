CREATE TABLE server_config(    
    contract_addr   text,

    -- enforce single record
    id              bool PRIMARY KEY DEFAULT true,
    CONSTRAINT      single_server_config CHECK (id)
);
