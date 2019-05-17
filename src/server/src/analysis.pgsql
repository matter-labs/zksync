-- update operations
-- set nonce = (id - 17) + (301 + 1), tx_hash = null
-- where id between 17 and 20;

-- update operations
-- set nonce = 1, tx_hash = 'busy'
-- where id > 20;

SELECT 
id, block_number, nonce, tx_hash
FROM "operations" 
order by id
LIMIT 1000;
