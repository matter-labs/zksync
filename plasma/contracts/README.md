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

## Local testing with `ganache-cli`

This will start ganache with 7M gas limit:

```
yarn ganache
```
