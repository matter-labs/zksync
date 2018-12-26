# Plasma Server

## Setup for testing

- Install postgres locally
- Install diesel-cli:

```cargo install diesel_cli --no-default-features --features postgres```

- From `server` dir run

```diesel database setup```

This will create database 'plasma' (db url is set in [.env](.env) file) with our schema.

- To reset migrations, run

```diesel migration redo```

- Run test to make sure everything works:

```cargo test --lib -- --nocapture test_store_state```

## Production

For production, `DATABSE_URL` env var must be set properly.
