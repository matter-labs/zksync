## ethjs

<div>
  <!-- Dependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs">
    <img src="https://david-dm.org/ethjs/ethjs.svg"
    alt="Dependency Status" />
  </a>

  <!-- devDependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs#info=devDependencies">
    <img src="https://david-dm.org/ethjs/ethjs/dev-status.svg" alt="devDependency Status" />
  </a>

  <!-- Build Status -->
  <a href="https://travis-ci.org/ethjs/ethjs">
    <img src="https://travis-ci.org/ethjs/ethjs.svg"
    alt="Build Status" />
  </a>

  <!-- NPM Version -->
  <a href="https://www.npmjs.org/package/ethjs">
    <img src="http://img.shields.io/npm/v/ethjs.svg"
    alt="NPM version" />
  </a>

  <!-- Test Coverage -->
  <a href="https://coveralls.io/r/ethjs/ethjs">
    <img src="https://coveralls.io/repos/github/ethjs/ethjs/badge.svg" alt="Test Coverage" />
  </a>

  <!-- Javascript Style -->
  <a href="http://airbnb.io/javascript/">
    <img src="https://img.shields.io/badge/code%20style-airbnb-brightgreen.svg" alt="js-airbnb-style" />
  </a>
</div>

<br />

A highly optimised, light-weight JS utility for [Ethereum](https://www.ethereum.org/) based on [`web3.js`](https://github.com/ethereum/web3.js), but lighter, async only and using `BN.js`.

Only **106 kB** minified!

## Install

```
npm install --save ethjs
```

## CDN

```
<script type="text/javascript" src="https://cdn.jsdelivr.net/npm/ethjs@0.3.4/dist/ethjs.min.js"></script>
```

Note, exports to `window.Eth` global.

## Usage

```js
const Eth = require('ethjs');
const eth = new Eth(new Eth.HttpProvider('https://ropsten.infura.io'));

eth.getBlockByNumber(45300, true, (err, block) => {
  // result null { ...block data... }
});

const etherValue = Eth.toWei(72, 'ether');

// result <BN: 3e733628714200000>

const tokenABI = [{
  "constant": true,
  "inputs": [],
  "name": "totalSupply",
  "outputs":[{"name": "","type": "uint256"}],
  "payable": false,
  "type": "function",
}];

const token = eth.contract(tokenABI).at('0x6e0E0e02377Bc1d90E8a7c21f12BA385C2C35f78');

token.totalSupply().then((totalSupply) => {
  // result <BN ...>  4500000
});

// token.transfer( ... ).then(txHash => eth.getTransactionSuccess(txHash)).then(receipt => console.log(receipt));
```

## About

A simple module for building dApps and applications that use Ethereum.

Please see our complete [`user-guide`](docs/user-guide.md) for more information.

## Contributing

Please help better the ecosystem by submitting issues and pull requests to `ethjs`. We need all the help we can get to build the absolute best linting standards and utilities. We follow the AirBNB linting standard and the unix philosophy.

## Guides

You'll find more detailed information on using `ethjs` and tailoring it to your needs in our guides:

- [User guide](docs/user-guide.md) - Usage, configuration, FAQ and complementary tools.
- [Developer guide](docs/developer-guide.md) - Contributing to `ethjs` and writing your own code and coverage.
- [Examples](http://github.com/ethjs/examples) - Examples of `ethjs` in use.

## Help out

There is always a lot of work to do, and will have many rules to maintain. So please help out in any way that you can:

- Create, enhance, and debug ethjs rules (see our guide to ["Working on rules"](./.github/CONTRIBUTING.md)).
- Improve documentation.
- Chime in on any open issue or pull request.
- Open new issues about your ideas for making `ethjs` better, and pull requests to show us how your idea works.
- Add new tests to *absolutely anything*.
- Create or contribute to ecosystem tools.
- Spread the word!

Please consult our [Code of Conduct](CODE_OF_CONDUCT.md) docs before helping out.

We communicate via [issues](https://github.com/ethjs/ethjs/issues) and [pull requests](https://github.com/ethjs/ethjs/pulls).

## Important documents

- [Changelog](CHANGELOG.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [License](https://raw.githubusercontent.com/ethjs/ethjs/master/LICENSE)

## Our Relationship with Ethereum & EthereumJS

We would like to mention that we are not in any way affiliated with the Ethereum Foundation. However, we love the work they do and work with them often to make Ethereum great! Our aim is to support the Ethereum ecosystem with a policy of diversity, modularity, simplicity, transparency, clarity, optimization and extensibility.

Many of our modules use code from `web3.js` and the `ethereumjs-` repositories. We thank the authors where we can in the relevant repositories.

## Special Thanks

`ethjs` was built by a strong community of Ethereum developers. A special thanks to:

- [Fabian Vogelsteller](https://twitter.com/feindura?lang=en) - for his work on `Mist` and `web3.js`
- [Tim Coulter](https://github.com/tcoulter) - for his work on `TestRPC` and `Truffle`
- [Aaron Davis](https://github.com/kumavis) - for his guidence and work on `MetaMask` and `ethereumjs`
- [Richard Moore](https://github.com/ricmoo) - for his work on `ethers-io` and `ethers-wallet` from which so much of `ethjs` is build from
- [Karl Floersch](https://twitter.com/karl_dot_tech?lang=en) - for his guidence and support
- [Martin B.](https://github.com/wanderer) - for his work on `ethereumjs`
- [Alex Beregszaszi](https://github.com/axic) - for his work on `ethereumjs`
- [Vitalik Buterin](https://twitter.com/VitalikButerin) - for creating `Ethereum`

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
