# Baby Plasma contracts

## Install truffle and dependencies:

```
yarn
```

## Re-compile contracts:

```
yarn build
```

IMPORTANT! Generated `.abi` and `.bin` files are fed to cargo to build module `plasma::eth`. 

So you need to rebuild the code on every change (to be automated soon).

## Local testing with `ganache-cli`

```
yarn ganache-cli
```

In another terminal:

```
yarn deploy-dev
```

Now contracts are available for local testing.