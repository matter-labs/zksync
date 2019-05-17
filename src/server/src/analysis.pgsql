SELECT 
id, block_number, nonce, tx_hash
FROM "operations" 
order by nonce
LIMIT 1000;