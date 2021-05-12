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
> cargo run --release -- -a ./sample/accounts -b ./sample/balances -h 0x2bd61f42837c0fa77fc113b3b341c520edb1ffadefc48c2b907901aaaf42b906 -p 0xd03f45dc6e06aa9a0fc53189a2a89561c42dc4ffffc13881d64401cd0beb604a

OldHash: 0x2bd61f42837c0fa77fc113b3b341c520edb1ffadefc48c2b907901aaaf42b906
NewHash: 0x2a9b50e17ece607c8c88b1833426fd9e60332685b94a1534fcf26948e373604c

Signing prefixed message: OldRootHash:0x2bd61f42837c0fa77fc113b3b341c520edb1ffadefc48c2b907901aaaf42b906,NewRootHash:0x2a9b50e17ece607c8c88b1833426fd9e60332685b94a1534fcf26948e373604c

Signature: 0x21ab9c91f12cc30146e7383d520002ec844eb614c888bb8c9deb628c95e516ed1061083996a80364a40b38125a9e380e64656293ed8b8591a4de9b064a235d901c
```
