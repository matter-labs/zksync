psql $DATABASE_URL -c "\copy (select id,nonce,address,pubkey_hash from accounts order by id) to 'accounts.csv' CSV HEADER"
psql $DATABASE_URL -c "\copy (select * from balances order by account_id,coin_id) to 'balances.csv' CSV HEADER"
psql $DATABASE_URL -c "\copy (select id,address from tokens order by id) to 'tokens.csv' CSV HEADER"

sed -i '' 's/\\x/0x/g' balances.csv
sed -i '' 's/\\x/0x/g' accounts.csv
sed -i '' 's/\\x/0x/g' tokens.csv
