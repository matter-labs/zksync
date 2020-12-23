# JavaScript SDK changelog
All notable changes to `zksync.js` will be documented in this file.

## Unreleased

## Prior to 2020-12-23

**Version 0.6.5** is released.

- Support of fast withdrawals was added. Corresponding optional field was added to the object passed to the
  `withdrawFromSyncToEthereum` method and `getTransactionFee` now accepts `FastWithdraw` fee type.

**Version 0.6.3** is released.

- Bundled version for browsers added. File `dist/main.js` can be used in `<script>` html tag. It requires global
  `ethers` object from [ethers-io/ethers.js](https://github.com/ethers-io/ethers.js/)
- `zksync.crypto.loadZkSyncCrypto()` method is added for browser builds that loads and compiles
  `zksync-crypto-web_bg.wasm` file. Should be called before any calls that use `zksync-crypto`.

**Version 0.6.0** is released.

- Upgrade ethers to ^5.0.0
