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
NewHash: 22aca7af1d99f525d1a60f31fd95f7626831a3a20561bf30c699da64149ac6b6
Signature: 21e38f19ca0d158a970c31db5a68c837ba69775cf58c9790eac870446659c00c0953e252d180cc81c288ac61e63724ccc7f85e21cff6d1f2cb180668c187e72000
```
