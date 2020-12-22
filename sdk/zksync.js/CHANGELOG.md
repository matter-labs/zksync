# JavaScript SDK changelog

### 0.6.5

- Support of fast withdrawals was added. Corresponding optional field was added to the object passed to the
  `withdrawFromSyncToEthereum` method and `getTransactionFee` now accepts `FastWithdraw` fee type.

### 0.6.3

- Bundled version for browsers added. File `dist/main.js` can be used in `<script>` html tag. It requires global
  `ethers` object from [ethers-io/ethers.js](https://github.com/ethers-io/ethers.js/)
- `zksync.crypto.loadZkSyncCrypto()` method is added for browser builds that loads and compiles
  `zksync-crypto-web_bg.wasm` file. Should be called before any calls that use `zksync-crypto`.

### 0.6.0

- Upgrade ethers to ^5.0.0

### zksync.js

- Support of WalletConnect was added.
