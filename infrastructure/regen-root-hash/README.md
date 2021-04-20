# Tool for regenerating root hash

This tool takes the JSON dump of `accounts` and `balances` from the zkSync database.

It regenerates the `OldRootHash` which is equal to the root hash of the account tree when each of the account state
subtrees has a depth of 11 and verifies that is the correct one. It also generates the `NewRootHash` which is equal to
the root hash of the account tree when each of the account state subtrees has a depth of 32. After re-verification that
the contents of the new tree are equivalent to the ones in the old tree, the following message is signed:
`OldRootHash:{OldRootHash},NewRootHash:{NewRootHash}`.

Run `cargo run -- --help` to get the help of the cli arguments that the program takes.

Note that the re-verification process is computation heavy, so running under `release` mode is recommended. Example of
using the tool:

```sh
> cargo run --release -- -a ./sample/accounts -b ./sample/balances -h 2bd61f42837c0fa77fc113b3b341c520edb1ffadefc48c2b907901aaaf42b906 -p d03f45dc6e06aa9a0fc53189a2a89561c42dc4ffffc13881d64401cd0beb604a

OldHash: 2bd61f42837c0fa77fc113b3b341c520edb1ffadefc48c2b907901aaaf42b906
NewHash: 09b11127828f5fcc4c0e18edbef20891c0028abce3b793f3262a196a1f63e487
Signature: 6d178d065f2d9fecfce00b77e45edf535198e28164e24632621a7105079e4f0d2aae29cb8b410f53f8d1121d26a095ebc79b3bd148c01c1e481e6a746a6e76ec01
```
