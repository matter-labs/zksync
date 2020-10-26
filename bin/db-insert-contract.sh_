#!/bin/bash

# Force read env -- this is important, sp that we re-ready the new contract value after redeploy!!!
ZKSYNC_ENV=
. .setup_env

psql "$DATABASE_URL" -c "INSERT INTO server_config (contract_addr, gov_contract_addr) \
                         VALUES ('$CONTRACT_ADDR', '$GOVERNANCE_ADDR') \
                         ON CONFLICT (id) DO UPDATE  \
                         SET (contract_addr, gov_contract_addr) = ('$CONTRACT_ADDR', '$GOVERNANCE_ADDR')" || exit 1
echo "successfully inserted contract address into the database"
