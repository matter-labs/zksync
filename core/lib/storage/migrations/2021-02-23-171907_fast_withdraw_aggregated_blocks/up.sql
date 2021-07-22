create table block_metadata (
    block_number bigserial NOT NULL REFERENCES blocks (number) on delete cascade,
    fast_processing  boolean not null,
    primary key (block_number)
);
