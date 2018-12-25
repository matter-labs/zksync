# Baby Plasma contracts

## Install truffle and dependencies:

```
yarn
```

## Re-build contracts:

```
yarn build
```

IMPORTANT! Generated `.abi` and `.bin` files are fed to cargo to build module `plasma::eth`. 

So you need to rebuild the code on every change (to be automated soon).

## Local testing with `geth`

```
yarn run geth
yarn run setup-geth
```

## Local testing with `ganache-cli`

Start ganache (the command below will start with 7M gas limit, chain 4 (simulating rinkeby) and predefined mnemonic):

```
yarn ganache
```

Run migration:

```
yarn truffle migrate --network dev
```

Export env var required by the `plasma::eth`:

```
export CHAIN_ID=4
export WEB3_URL=http://localhost:8545
export SENDER_ACCOUNT=e5d0efb4756bd5cdd4b5140d3d2e08ca7e6cf644
export PRIVATE_KEY=aa8564af9bef22f581e99125d1829b76c45d08e4f6f0b74d586911f4318b6776 
export CONTRACT_ADDR=81e872C3DF5c32DDbE3391c0427BEAEB985aAA31
```

## Deploying to Rinkeby testnet

export MNEMONIC="<your metamask HD mnemonic>"
export INFURA_PROJECT_ID=<infura_projec_id>

```
yarn truffle migrate --rinkeby
```
