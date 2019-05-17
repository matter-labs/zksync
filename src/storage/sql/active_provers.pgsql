-- update active_provers
-- set stopped_at = now()
-- where id <= 118;

select count(*) from active_provers
where stopped_at is null
;

with pr as (
SELECT 
    *,
    EXTRACT(epoch FROM (updated_at - created_at)) as since
FROM prover_runs pr
WHERE NOT EXISTS (SELECT * FROM proofs p WHERE p.block_number = pr.block_number)
ORDER BY id desc
)
select *
from pr
order by block_number asc;
