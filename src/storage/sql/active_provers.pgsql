SELECT * FROM prover_runs pr
WHERE NOT EXISTS (SELECT * FROM proofs p WHERE p.block_number = pr.block_number)
ORDER BY id desc;