# zkSync SDK test vector generator

An utility to generate deterministic test vectors for various SDK. By having all the SDK share the same test vectors,
it's easier to ensure that behavior of all the implementations is correct and consistent.

## Launching

```bash
yarn
yarn generate
```

Result test vector will be created in the package directory. Output file name is `test-vectors.json`.
