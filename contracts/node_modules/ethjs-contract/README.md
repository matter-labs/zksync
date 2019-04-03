## ethjs-contract

<div>
  <!-- Dependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs-contract">
    <img src="https://david-dm.org/ethjs/ethjs-contract.svg"
    alt="Dependency Status" />
  </a>

  <!-- devDependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs-contract#info=devDependencies">
    <img src="https://david-dm.org/ethjs/ethjs-contract/dev-status.svg" alt="devDependency Status" />
  </a>

  <!-- Build Status -->
  <a href="https://travis-ci.org/ethjs/ethjs-contract">
    <img src="https://travis-ci.org/ethjs/ethjs-contract.svg"
    alt="Build Status" />
  </a>

  <!-- NPM Version -->
  <a href="https://www.npmjs.org/package/ethjs-contract">
    <img src="http://img.shields.io/npm/v/ethjs-contract.svg"
    alt="NPM version" />
  </a>

  <!-- Test Coverage -->
  <a href="https://coveralls.io/r/ethjs/ethjs-contract">
    <img src="https://coveralls.io/repos/github/ethjs/ethjs-contract/badge.svg" alt="Test Coverage" />
  </a>

  <!-- Javascript Style -->
  <a href="http://airbnb.io/javascript/">
    <img src="https://img.shields.io/badge/code%20style-airbnb-brightgreen.svg" alt="js-airbnb-style" />
  </a>
</div>

<br />

A simple contract module for the Ethereum RPC layer.

NOTE. Module not ready for use, still in heavy development.

## Install

```
npm install --save ethjs-contract
```

## Usage

```js
const HttpProvider = require('ethjs-provider-http');
const Eth = require('ethjs-query');
const EthContract = require('ethjs-contract');
const eth = new Eth(new HttpProvider('http://localhost:8545'));
const contract = new EthContract(eth);

const SimpleStore = contract(abi, bytecode, defaultTxObject);
const simpleStore = SimpleStore.at('0x000...');
const simpleStore = SimpleStore.new((error, result) => {
  // result null '0x928sdfk...'
});

simpleStore.set(45000, (error, result) => {
  // result null '0x2dfj24...'
});

simpleStore.get().catch((error) => {
  // error null
}).then(result) => {
  // result <BigNumber ...>
});

const filter = simpleStore.SetComplete()
.new({ toBlock: 'latest' }, (error, result) => {
  // result null <BigNumber ...> filterId
});
filter.watch((err, result) => {
  // result null FilterResult {...}
});
filter.uninstall()
.then(result) => {
  // result Boolean
});
```

## About

A simple contract object for the Ethereum RPC layer.

## Contributing

Please help better the ecosystem by submitting issues and pull requests to default. We need all the help we can get to build the absolute best linting standards and utilities. We follow the AirBNB linting standard and the unix philosophy.

## Guides

You'll find more detailed information on using `ethjs-contract` and tailoring it to your needs in our guides:

- [User guide](docs/user-guide.md) - Usage, configuration, FAQ and complementary tools.
- [Developer guide](docs/developer-guide.md) - Contributing to `ethjs-contract` and writing your own code and coverage.

## Help out

There is always a lot of work to do, and will have many rules to maintain. So please help out in any way that you can:

- Create, enhance, and debug ethjs rules (see our guide to ["Working on rules"](./github/CONTRIBUTING.md)).
- Improve documentation.
- Chime in on any open issue or pull request.
- Open new issues about your ideas for making `ethjs-contract` better, and pull requests to show us how your idea works.
- Add new tests to *absolutely anything*.
- Create or contribute to ecosystem tools, like modules for encoding or contracts.
- Spread the word.

Please consult our [Code of Conduct](CODE_OF_CONDUCT.md) docs before helping out.

We communicate via [issues](https://github.com/ethjs/ethjs-contract/issues) and [pull requests](https://github.com/ethjs/ethjs-contract/pulls).

## Important documents

- [Changelog](CHANGELOG.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [License](https://raw.githubusercontent.com/ethjs/ethjs-contract/master/LICENSE)

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
