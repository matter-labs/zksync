# Utility to send Exodus transaction

This utility provides a sample script to perform exodus for an account that has a known private key. In case you use
wallet that does not expose private key, consult your wallet documentation in order to know how to execute arbitrary
transactions from it.

Prior to usage, you must copy JSON output of `exit-tool` script to some file, named, for example, `input.json`.

Usage:

```
yarn
yarn build
yarn start -pk <your private key> -t <zkSync contract address> -n <Ethereum network> -p <path to input JSON file>
```
