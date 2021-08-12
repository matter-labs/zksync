# Generating new circuit keys

Prover required circuit keys. You can generate them manually.

## 0. Requirements

RAM: 128 GB 

CPU: 32 CPUs

## 1. Preparation

Clone repository to your machine

## 2. Installing dependencies

Key generation required the same [dependencies](../docs/setup-dev.md)  as our server.

## 4. Generating keys

```bash
zk
zk run plonk-setup download
zk run verify-keys gen
# Note: etc/env/dev.env.example must have correct keys/DIR
zk run verify-keys pack
```

After that, generated packed key will be placed into `keys/packed` directory. You may either copy this file to your
local machine, or commit it directly to the repo.
