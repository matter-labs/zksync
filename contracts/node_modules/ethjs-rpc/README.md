## ethjs-rpc

<div>
  <!-- Dependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs-rpc">
    <img src="https://david-dm.org/ethjs/ethjs-rpc.svg"
    alt="Dependency Status" />
  </a>

  <!-- devDependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs-rpc#info=devDependencies">
    <img src="https://david-dm.org/ethjs/ethjs-rpc/dev-status.svg" alt="devDependency Status" />
  </a>

  <!-- Build Status -->
  <a href="https://travis-ci.org/ethjs/ethjs-rpc">
    <img src="https://travis-ci.org/ethjs/ethjs-rpc.svg"
    alt="Build Status" />
  </a>

  <!-- NPM Version -->
  <a href="https://www.npmjs.org/package/ethjs-rpc">
    <img src="http://img.shields.io/npm/v/ethjs-rpc.svg"
    alt="NPM version" />
  </a>

  <!-- Test Coverage -->
  <a href="https://coveralls.io/r/ethjs/ethjs-rpc">
    <img src="https://coveralls.io/repos/github/ethjs/ethjs-rpc/badge.svg" alt="Test Coverage" />
  </a>

  <!-- Javascript Style -->
  <a href="http://airbnb.io/javascript/">
    <img src="https://img.shields.io/badge/code%20style-airbnb-brightgreen.svg" alt="js-airbnb-style" />
  </a>
</div>

<br />

A super simple module for querying the Ethereum RPC layer without formatting.

This module was influenced by: [`eth-query`](https://github.com/ethereumjs/eth-query).

## Install

```
npm install --save ethjs-rpc
```

## Usage

```js
const HttpProvider = require('ethjs-provider-http');
const EthRPC = require('ethjs-rpc');
const eth = new EthRPC(new HttpProvider('http://localhost:8545'));

// accounts
eth.sendAsync({ method: 'eth_accounts' }, (err, accounts1) => {
  // null ['0x...', '0x....']
});

// gasPrice
eth.sendAsync({ method: 'eth_gasPrice' }, (err, gasPrice) => {
  // null '0xe83922'
});

// getBalance
eth.sendAsync({
  method: 'eth_getBalance',
  params: ['0x5c517fffeacc03caaa8f0aa5722168b9ab3fb7a1', 'latest'],
}, (err, gasPrice) => {
  // null '0x5483de922'
});
```

## About

This simple module allows you to query the Ethereum RPC layer at a very low level. You must specify your data payload method and params, if any. The module will handle payload RPC ids.

## Contributing

Please help better the ecosystem by submitting issues and pull requests to `ethjs-rpc`. We need all the help we can get to build the absolute best linting standards and utilities. We follow the AirBNB linting standard and the unix philosophy.

## Guides

You'll find more detailed information on using `ethjs-rpc` and tailoring it to your needs in our guides:

- [User guide](docs/user-guide.md) - Usage, configuration, FAQ and complementary tools.
- [Developer guide](docs/developer-guide.md) - Contributing to `ethjs-rpc` and writing your own code and coverage.

## Help out

There is always a lot of work to do, and will have many rules to maintain. So please help out in any way that you can:

- Create, enhance, and debug ethjs rules (see our guide to ["Working on rules"](./github/CONTRIBUTING.md)).
- Improve documentation.
- Chime in on any open issue or pull request.
- Open new issues about your ideas for making `ethjs-rpc` better, and pull requests to show us how your idea works.
- Add new tests to *absolutely anything*.
- Create or contribute to ecosystem tools, like modules for encoding or contracts.
- Spread the word.

Please consult our [Code of Conduct](CODE_OF_CONDUCT.md) docs before helping out.

We communicate via [issues](https://github.com/ethjs/ethjs-rpc/issues) and [pull requests](https://github.com/ethjs/ethjs-rpc/pulls).

## Important documents

- [Changelog](CHANGELOG.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [License](https://raw.githubusercontent.com/ethjs/ethjs-rpc/master/LICENSE)

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
