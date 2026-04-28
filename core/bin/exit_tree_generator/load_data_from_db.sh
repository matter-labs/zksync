psql $DATABASE_URL -c "\copy (select id,nonce,address,pubkey_hash from accounts order by id) to 'data/accounts.csv' CSV HEADER"
psql $DATABASE_URL -c "\copy (select * from balances order by account_id,coin_id) to 'data/balances.csv' CSV HEADER"
psql $DATABASE_URL -c "\copy (select id,address,symbol,decimals from tokens order by id) to 'data/tokens.csv' CSV HEADER"

sed -i '' 's/\\x/0x/g' data/balances.csv
sed -i '' 's/\\x/0x/g' data/accounts.csv
sed -i '' 's/\\x/0x/g' data/tokens.csv
