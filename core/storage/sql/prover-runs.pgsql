-- with t as (
-- select 
--     block_number, count(*)
-- from prover_runs
-- group by block_number
-- )

-- select * from t order by count desc
-- limit 100;

select * from prover_runs 
--where block_number = 6;
order by id
limit 50;

select block_number, created_at from proofs
where block_number = 6;
