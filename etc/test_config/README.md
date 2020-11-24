# Test data for zkSync

This folder contains the data required for various zkSync tests.

Directory contains three subfolders:

- `constant`: Data that remains the same between various runs, filled manually and committed to the repository. For
  example, private / public keys of test accounts.
- `volatile`: Data that may change, filled by scripts and is **not** committed to the repository. For example, deployed
  contracts addresses.
- `sdk`: Data used to test SDK implementations.
