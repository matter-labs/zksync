with d as (
with dat as (
with blocks as (
    select
        block_number, action_type, created_at,
        26 as from_block, -- 12
        70 as to_block   -- 265
    from operations
    where
        action_type = 'VERIFY'
        and data->'block'->'block_data'->>'type' = 'Transfer'
    order by block_number desc
)
select
    count(*) as n,
    (select created_at from blocks where block_number = from_block) as from_time,
    (select created_at from blocks where block_number = to_block) as to_time
from blocks
where block_number >= from_block and block_number <= to_block
) select *, n * 256 as txs, EXTRACT(epoch FROM (to_time - from_time)) as seconds from dat
) select *, txs / seconds as tps from d;
