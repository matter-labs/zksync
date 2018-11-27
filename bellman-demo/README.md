# Demo circuits

This project contains usage demonstration for `bellman` zkSNARK proving framework.  
We use elliptic curve BN256, for which pairings can be efficiently performed in Ethereum Virtual Machine.

## Project structure

```
.
│ 
├── examples
│   ├── xor.rs: simple XOR circuit demo
│   └── sha256.rs: sha256 preimage demo (using gadgets from `sapling-crypto` crate)
└── src
    ├── bin
    │   └── bench.rs: benchmarking tool
    └── lib.rs: demo contract rendering
```

## Usage:

```$bash
cargo run --example xor
cargo run --example sha256
cargo run --bin circuit
```

## Verification in EVM contract:

```$bash
cargo run --example sha256 > demo.sol
```

Now deploy `DemoVerifier` contract from `demo.sol` (e.g. in [remix](https://remix.ethereum.org)) and run method `verify()`.

## Benchmarking

```$bash
BELLMAN_VERBOSE=1 cargo run --release [num_constraints]
```

`num_constraints` is decimal:

```$bash
BELLMAN_VERBOSE=1 cargo run --release 1000000
```