# the testing tool

### Usage: 
To launch one of the tests, from `franklin/js/franklin_lib` run `f yarn ts-node scripts/loadtest/{test}.ts`.
Or copy one of files in `/tests` and change the operations added.

### Adding operations:

``` ts
// each arg gets some random value if not specified.
let op = tester.randomDepositOperation({
    wallet: tester.wallets[0], 
    token: tester.tokens[0], 
    amount: bigNumberify('100000')
});
// add the operation to wallet so that it will be run.
tester.addOperation(op);
```

