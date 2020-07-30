#!/bin/bash
# db-insert-token.sh id, address, symbol, precision

# Force read env -- this is important, sp that we re-ready the new contract value after redeploy!!!
ZKSYNC_ENV=
. .setup_env

psql "$DATABASE_URL" -c "INSERT INTO tokens \
                         VALUES ($1, '$2', '$3', $4);" || exit 1
echo "successfully inserted token into the database"
