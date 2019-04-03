## ethjs-query

<div>
  <!-- Dependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs-query">
    <img src="https://david-dm.org/ethjs/ethjs-query.svg"
    alt="Dependency Status" />
  </a>

  <!-- devDependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs-query#info=devDependencies">
    <img src="https://david-dm.org/ethjs/ethjs-query/dev-status.svg" alt="devDependency Status" />
  </a>

  <!-- Build Status -->
  <a href="https://travis-ci.org/ethjs/ethjs-query">
    <img src="https://travis-ci.org/ethjs/ethjs-query.svg"
    alt="Build Status" />
  </a>

  <!-- NPM Version -->
  <a href="https://www.npmjs.org/package/ethjs-query">
    <img src="http://img.shields.io/npm/v/ethjs-query.svg"
    alt="NPM version" />
  </a>

  <!-- Test Coverage -->
  <a href="https://coveralls.io/r/ethjs/ethjs-query">
    <img src="https://coveralls.io/repos/github/ethjs/ethjs-query/badge.svg" alt="Test Coverage" />
  </a>

  <!-- Javascript Style -->
  <a href="http://airbnb.io/javascript/">
    <img src="https://img.shields.io/badge/code%20style-airbnb-brightgreen.svg" alt="js-airbnb-style" />
  </a>
</div>

<br />

A simple module for querying the Ethereum RPC layer.

## Install

```
npm install --save ethjs-query
```

## Usage

```js
const BN = require('bn.js');
const HttpProvider = require('ethjs-provider-http');
const Eth = require('ethjs-query');
const eth = new Eth(new HttpProvider('http://localhost:8545'));

eth.getBalance('0x407d73d8a49eeb85d32cf465507dd71d507100c1', cb);

// result null <BN ...>

eth.sendTransaction({
  from: '0x407d73d8a49eeb85d32cf465507dd71d507100c1',
  to: '0x987d73d8a49eeb85d32cf462207dd71d50710033',
  value: new BN('6500000'),
  gas: 3000000,
  data: '0x',
}).then(cb).catch(cb);

// result null 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
```

## About

A simple Ethereum RPC module for querying data from an Ethereum node such as a geth (go-etherem), parity (rust-ethereum) or TestRPC (local js-ethereum).

This module supports all Ethereum RPC methods and is designed completely to specification.

## Amorphic Data Formatting

`ethjs-query` uses the `ethjs-format` module to format incoming and outgoing RPC data payloads. The primary formatting task is numbers. Number values can be inputed as: `BigNumber`, `BN`, `string`, `hex` or `actual numbers`. Because the blockchain does not support decimal or negative numbers, any kind of decimal or negative number will cause an error return. All received number values are returned as BN.js object instances.

Read more about the formatting layer here: [ethjs-format](http://github.com/ethjs/ethjs-format)

## Async Only

All methods are `async` only, requiring either a callback or promise.

## Error handling

Error handling is done through function callbacks or promised catches.

## Debugging Options

`ethjs-query` comes equip with a full debug options for all data inputs and outputs.

```js
const HttpProvider = require('ethjs-provider-http');
const Eth = require('ethjs-query');
const eth = new Eth(new HttpProvider('http://localhost:8545'), { debug: true, logger: console, jsonSpace: 0 });

eth.accounts(cb);

/* result
[ethjs-query 2016-11-27T19:37:54.917Z] attempting method accounts with params [null]
[ethjs-query 2016-11-27T19:37:54.917Z] [method 'accounts'] callback provided: true
[ethjs-query 2016-11-27T19:37:54.917Z] [method 'accounts'] attempting input formatting of 0 inputs
[ethjs-query 2016-11-27T19:37:54.917Z] [method 'accounts'] formatted inputs: []
[ethjs-query 2016-11-27T19:37:54.917Z] [method 'accounts'] attempting query with formatted inputs...
[ethjs-query 2016-11-27T19:37:54.919Z] [method 'accounts'] callback success, attempting formatting of raw outputs: ["0xb88643569c19d05dc67b960f91d9d696eebf808e","0xf...]
[ethjs-query 2016-11-27T19:37:54.919Z] [method 'accounts'] formatted outputs: ["0xb88643569c19d05dc67b960f91d9d696eebf808e","0xf...]
*/
```

## Supported Methods

`ethjs-query` supports all Ethereum specified RPC methods.

```js
const HttpProvider = require('ethjs-provider-http');
const Eth = require('ethjs-query');
const eth = new Eth(new HttpProvider('http://localhost:8545'));

eth.protocolVersion(cb);

// ....
```

* [eth.protocolVersion](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_protocolversion)
* [eth.syncing](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_syncing)
* [eth.coinbase](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_coinbase)
* [eth.mining](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_mining)
* [eth.hashrate](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_hashrate)
* [eth.gasPrice](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_gasprice)
* [eth.accounts](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_accounts)
* [eth.blockNumber](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_blocknumber)
* [eth.getBalance](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getbalance)
* [eth.getStorageAt](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getstorageat)
* [eth.getTransactionCount](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_gettransactioncount)
* [eth.getBlockTransactionCountByHash](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getblocktransactioncountbyhash)
* [eth.getBlockTransactionCountByNumber](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getblocktransactioncountbynumber)
* [eth.getUncleCountByBlockHash](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getunclecountbyblockhash)
* [eth.getUncleCountByBlockNumber](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getunclecountbyblocknumber)
* [eth.getCode](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getcode)
* [eth.sign](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_sign)
* [eth.sendTransaction](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_sendtransaction)
* [eth.sendRawTransaction](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_sendrawtransaction)
* [eth.call](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_call)
* [eth.estimateGas](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_estimategas)
* [eth.getBlockByHash](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getblockbyhash)
* [eth.getBlockByNumber](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getblockbynumber)
* [eth.getTransactionByHash](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_gettransactionbyhash)
* [eth.getTransactionByBlockHashAndIndex](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_gettransactionbyblockhashandindex)
* [eth.getTransactionByBlockNumberAndIndex](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_gettransactionbyblocknumberandindex)
* [eth.getTransactionReceipt](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_gettransactionreceipt)
* [eth.getUncleByBlockHashAndIndex](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getunclebyblockhashandindex)
* [eth.getUncleByBlockNumberAndIndex](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getunclebyblocknumberandindex)
* [eth.getCompilers](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getcompilers)
* [eth.compileLLL](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_compilelll)
* [eth.compileSolidity](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_compilesolidity)
* [eth.compileSerpent](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_compileserpent)
* [eth.newFilter](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_newfilter)
* [eth.newBlockFilter](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_newblockfilter)
* [eth.newPendingTransactionFilter](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_newpendingtransactionfilter)
* [eth.uninstallFilter](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_uninstallfilter)
* [eth.getFilterChanges](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getfilterchanges)
* [eth.getFilterLogs](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getfilterlogs)
* [eth.getLogs](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getlogs)
* [eth.getWork](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_getwork)
* [eth.submitWork](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_submitwork)
* [eth.submitHashrate](https://github.com/ethereum/wiki/wiki/JSON-RPC#eth_submithashrate)

* [eth.web3_clientVersion](https://github.com/ethereum/wiki/wiki/JSON-RPC#web3_clientversion)
* [eth.web3_sha3](https://github.com/ethereum/wiki/wiki/JSON-RPC#web3_sha3)

* [eth.net_version](https://github.com/ethereum/wiki/wiki/JSON-RPC#net_version)
* [eth.net_peerCount](https://github.com/ethereum/wiki/wiki/JSON-RPC#net_peercount)
* [eth.net_listening](https://github.com/ethereum/wiki/wiki/JSON-RPC#net_listening)

* [eth.db_putString](https://github.com/ethereum/wiki/wiki/JSON-RPC#db_putstring)
* [eth.db_getString](https://github.com/ethereum/wiki/wiki/JSON-RPC#db_getstring)
* [eth.db_putHex](https://github.com/ethereum/wiki/wiki/JSON-RPC#db_puthex)
* [eth.db_getHex](https://github.com/ethereum/wiki/wiki/JSON-RPC#db_gethex)

* [eth.shh_post](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_post)
* [eth.shh_version](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_version)
* [eth.shh_newIdentity](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_newidentity)
* [eth.shh_hasIdentity](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_hasidentity)
* [eth.shh_newGroup](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_newgroup)
* [eth.shh_addToGroup](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_addtogroup)
* [eth.shh_newFilter](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_newfilter)
* [eth.shh_uninstallFilter](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_uninstallfilter)
* [eth.shh_getFilterChanges](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_getfilterchanges)
* [eth.shh_getMessages](https://github.com/ethereum/wiki/wiki/JSON-RPC#shh_getmessages)

## Contributing

Please help better the ecosystem by submitting issues and pull requests to `ethjs-query`. We need all the help we can get to build the absolute best linting standards and utilities. We follow the AirBNB linting standard and the unix philosophy.

## Guides

You'll find more detailed information on using `ethjs-query` and tailoring it to your needs in our guides:

- [User guide](docs/user-guide.md) - Usage, configuration, FAQ and complementary tools.
- [Developer guide](docs/developer-guide.md) - Contributing to `ethjs-query` and writing your own code and coverage.

## Help out

There is always a lot of work to do, and will have many rules to maintain. So please help out in any way that you can:

- Create, enhance, and debug ethjs rules (see our guide to ["Working on rules"](./github/CONTRIBUTING.md)).
- Improve documentation.
- Chime in on any open issue or pull request.
- Open new issues about your ideas for making `ethjs-query` better, and pull requests to show us how your idea works.
- Add new tests to *absolutely anything*.
- Create or contribute to ecosystem tools, like modules for encoding or contracts.
- Spread the word.

Please consult our [Code of Conduct](CODE_OF_CONDUCT.md) docs before helping out.

We communicate via [issues](https://github.com/ethjs/ethjs-query/issues) and [pull requests](https://github.com/ethjs/ethjs-query/pulls).

## Important documents

- [Changelog](CHANGELOG.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [License](https://raw.githubusercontent.com/ethjs/ethjs-query/master/LICENSE)

## Licence

This project is licensed under the MIT license, Copyright (c) 2016 Nick Dodson. For more information see LICENSE.md.

```
The MIT License

Copyright (c) 2016 Nick Dodson. nickdodson.com

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```
