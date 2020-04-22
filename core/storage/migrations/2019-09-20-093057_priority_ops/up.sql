create table executed_priority_operations
(
    id                   serial primary key,
    -- sidechain block info
    block_number         bigint  not null,
    block_index          int     not null,
    -- operation data
    operation            jsonb   not null,
    -- operation metadata
    priority_op_serialid bigint  not null,
    deadline_block       bigint  not null
);


ALTER TABLE executed_transactions
    ADD COLUMN block_index int;

ALTER TABLE operations
    DROP COLUMN data;

create table blocks
(
    number                      bigint primary key,
    root_hash                   text   not null,
    fee_account_id              bigint not null,
    unprocessed_prior_op_before bigint not null,
    unprocessed_prior_op_after  bigint not null
);
