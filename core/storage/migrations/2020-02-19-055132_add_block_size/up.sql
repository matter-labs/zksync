alter table blocks
add block_size bigint not null;

alter table active_provers
add block_size bigint not null;
