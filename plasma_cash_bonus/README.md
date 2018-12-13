# Plasma cash history SNARK

Compacts history in Plasma Cashes by hiding Merkle proofs under the private witness. Currently contains only non-inclusion circuit, with inclusion being trivially extended.

Without much optimization is requires 4270718 constraints for 128 block of non-inclusion for 24 tree depth.

Public inputs to the zkSNARK:

- Start of the interval index (if single coin - just index)
- Interval length (is single coin - 1)
- Set of roots for which this coin index is proved to be non-included

## Notice

SNARK checks that start of the interval is divisible by the interval length, but in principle such check should be done outside of the snark as range start and length are public inputs.

## Run

Dummy tree and proof are generated for a large set of blocks
```
cargo run --release --bin benchmark_proof_gen
```

You can also sent an environment variable `BELLMAN_VERBOSE=1` to have some verbose setup and proof generation progress.

## Benchmark
```
    Using test constraint system to check the satisfiability
    Synthsizing a snark for 128 block for 24 tree depth
    Looking for unconstrained variabled:
    Number of constraints = 4263710
    generating setup...
    Has generated 4263684 points
    setup generated in 358.876 s
    creating proof...
    proof created in 39.749 s
    Proof is valid
```

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

