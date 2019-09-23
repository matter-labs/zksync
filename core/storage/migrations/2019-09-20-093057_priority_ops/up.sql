create table executed_priority_operations
(
    id                   serial primary key,
    -- sidechain block info
    block_number         bigint  not null,
    block_index          int     not null,
    -- operation data
    operation            jsonb not null,
    -- operation metadata
    priority_op_serialid bigint  not null,
    eth_fee              numeric not null
);


ALTER TABLE executed_transactions
    ADD COLUMN block_index int;
