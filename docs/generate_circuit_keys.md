# Generating new circuit keys

Prover requires circuit keys. You can generate them manually.

## 0. Requirements

RAM: 196 GB

CPU: 32 CPUs

## 1. Preparation

Clone the repository to your machine

## 2. Installing dependencies

Key generation requires the same [dependencies](../docs/setup-dev.md) as our server.

## 4. Generating keys

```bash
zk
zk run plonk-setup download
zk run verify-keys gen
# Note: generated etc/env/dev.env must have the correct keys/DIR
zk run verify-keys pack
```

After that, generated packed key will be placed into the `keys/packed` directory. You may either copy this file to your
local machine, or commit it directly to the repo.
