-- update operations
-- set nonce = (id - 17) + (301 + 1), tx_hash = null
-- where id between 17 and 20;

-- update operations
-- set tx_hash = '0x2e8a98ec143d2db6058e09fd144c4c9c69c0d13e37161a23dc950ad14ddf8d37'
-- -- set nonce = 1, tx_hash = 'busy'
-- where id = 268 and tx_hash = '0x024fc83bd5ff98a4b10a2cce3d757d212ed234c5ecf800bbf7899d4c7df779e6';

SELECT 
id, block_number, nonce, tx_hash
FROM "operations" 
order by nonce desc
LIMIT 1000;
