# `zcli` - command line interface to zkSync

## Configuration

Config file `.zcli-config.json` is auto-generated and is not to be edited.

Config management can be done through the CLI.

By default, `zcli` tries to open `./.zcli-config.json`. If not present,
`$ZCLI_HOME/.zcli-config.json`.

## Usage

All output printed to `stdout` is strict `JSON` and parseable by `jq`.

`--help` and `--version` options are available (not `JSON`).

If any error occures, process exits with error code 1.

### Wallet management

All wallets are stored as unencrypted private keys in `.zcli-config.json`.
One of them may be set as default wallet.

```bash
# lists all wallets' addresses
zcli wallets

# prints address of the default wallet (or null)
zcli wallets default

# sets ADDRESS as a default wallet
zcli wallets default ADDRESS

# adds a wallet to config
# if key is not provided, creates a random wallet
# if default wallet was not set - sets it as default
zcli wallets add [PRIVATE_KEY]

# removes ADDRESS from wallets
zcli wallets delete ADDRESS
```

### Network management

In every command, default network may be overriden by `-n NETWORK` flag.
`NETWORK` can be either `localhost`, `rinkeby`, `ropsten` or `mainnet`.

```bash
# list available networks
zcli networks

# print default network
zcli networks default

# set default network to NETWORK
zcli networks default NETWORK
```

### Fetching information

```bash
# prints info about account - nonce, id, balances etc.
# by default ADDRESS is set to default wallet
zcli account [ADDRESS]

# prints info about transaction - from, to, amount, token, fee etc.
zcli transaction TX_HASH

# same as transaction, but first waits until it's commited/verified
# -t flag supplies timeout, after which the command returns null (default: 60)
zcli await commit [-t SECONDS] TX_HASH
zcli await verify [-t SECONDS] TX_HASH
```

### Creating transactions

```bash
# makes a deposit from default wallet to ADDRESS
# by default ADDRESS is set to default wallet
zcli deposit [--fast] AMOUNT TOKEN [ADDRESS]

# makes a deposit from wallet with PRIVATE_KEY to ADDRESS
zcli deposit [--fast] --json '{ amount: AMOUNT, token: TOKEN, from: PRIVATE_KEY, to: ADDRESS }'

# makes a transfer from default wallet to ADDRESS
zcli transfer [--fast] AMOUNT TOKEN ADDRESS

# makes a deposit from wallet with PRIVATE_KEY to ADDRESS
zcli transfer [--fast] --json '{ amount: AMOUNT, token: TOKEN, from: PRIVATE_KEY, to: ADDRESS }'
```

If `--fast` is set, the command will not wait for transaction commitment and only print the transaction hash.
Otherwise, full information about transaction is printed.


## Installation

After (if) this package is published to `npm`, installation is as easy as

```bash
yarn global add zcli
```

## Example usage

```
$ zcli netwokrks default ropsten
"ropsten"
$ zcli wallets add 0x8888888888888888888888888888888888888888888888888888888888888888
"0x62f94E9AC9349BCCC61Bfe66ddAdE6292702EcB6"
$ zcli deposit 3.14 ETH
{
    "network": "ropsten",
    "transaction": {
        "status": "success",
        "from": "0x62f94E9AC9349BCCC61Bfe66ddAdE6292702EcB6",
        "to": "0x62f94E9AC9349BCCC61Bfe66ddAdE6292702EcB6",
        "hash": "0x602de5abbdbb9ab1861cf04c8580e8b0d6bee9f16d6dfbf2c08d8aa624241115",
        "operation": "Deposit",
        "nonce": -1,
        "amount": "3.14",
        "token": "ETH"
    }
}
$ zcli transfer --fast 3.0 ETH 0x36615cf349d7f6344891b1e7ca7c72883f5dc049
"sync-tx:f945ace556a6576e05c38a0fcca29f40674ea9a14d49c099b51a12737d9dac7b"
$ zcli account 0x36615cf349d7f6344891b1e7ca7c72883f5dc049
{
    "network": "ropsten",
    "address": "0x36615cf349d7f6344891b1e7ca7c72883f5dc049",
    "account_id": 5,
    "nonce": 2,
    "balances": {
        "ETH": "3.0"
    }
}
```
