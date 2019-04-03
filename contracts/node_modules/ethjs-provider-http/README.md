## ethjs-provider-http

<div>
  <!-- Dependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs-provider-http">
    <img src="https://david-dm.org/ethjs/ethjs-provider-http.svg"
    alt="Dependency Status" />
  </a>

  <!-- devDependency Status -->
  <a href="https://david-dm.org/ethjs/ethjs-provider-http#info=devDependencies">
    <img src="https://david-dm.org/ethjs/ethjs-provider-http/dev-status.svg" alt="devDependency Status" />
  </a>

  <!-- Build Status -->
  <a href="https://travis-ci.org/ethjs/ethjs-provider-http">
    <img src="https://travis-ci.org/ethjs/ethjs-provider-http.svg"
    alt="Build Status" />
  </a>

  <!-- NPM Version -->
  <a href="https://www.npmjs.org/package/ethjs-provider-http">
    <img src="http://img.shields.io/npm/v/ethjs-provider-http.svg"
    alt="NPM version" />
  </a>

  <!-- Test Coverage -->
  <a href="https://coveralls.io/r/ethjs/ethjs-provider-http">
    <img src="https://coveralls.io/repos/github/ethjs/ethjs-provider-http/badge.svg" alt="Test Coverage" />
  </a>

  <!-- Javascript Style -->
  <a href="http://airbnb.io/javascript/">
    <img src="https://img.shields.io/badge/code%20style-airbnb-brightgreen.svg" alt="js-airbnb-style" />
  </a>
</div>

<br />

A simple provider module for the Ethereum RPC layer.

## Install

```
npm install --save ethjs-provider-http
```

## Usage

```js
const HttpProvider = require('ethjs-provider-http');
const Eth = require('ethjs-query');
const eth = new Eth(new HttpProvider('https://ropsten.infura.io'));

eth.getBlockByNumber(45039930, cb);

// result null { hash: 0x.. etc.. }
});
```

## About

A simple provider module for the HTTP protocol that follows the web3 provider specification. This is purely an HTTP bypass using the XHR2 module.

## Contributing

Please help better the ecosystem by submitting issues and pull requests to default. We need all the help we can get to build the absolute best linting standards and utilities. We follow the AirBNB linting standard and the unix philosophy.

## Guides

You'll find more detailed information on using `ethjs-provider-http` and tailoring it to your needs in our guides:

- [User guide](docs/user-guide.md) - Usage, configuration, FAQ and complementary tools.
- [Developer guide](docs/developer-guide.md) - Contributing to `ethjs-provider-http` and writing your own code and coverage.

## Help out

There is always a lot of work to do, and will have many rules to maintain. So please help out in any way that you can:

- Create, enhance, and debug ethjs rules (see our guide to ["Working on rules"](./github/CONTRIBUTING.md)).
- Improve documentation.
- Chime in on any open issue or pull request.
- Open new issues about your ideas for making `ethjs-provider-http` better, and pull requests to show us how your idea works.
- Add new tests to *absolutely anything*.
- Create or contribute to ecosystem tools, like modules for encoding or contracts.
- Spread the word.

Please consult our [Code of Conduct](CODE_OF_CONDUCT.md) docs before helping out.

We communicate via [issues](https://github.com/ethjs/ethjs-provider-http/issues) and [pull requests](https://github.com/ethjs/ethjs-provider-http/pulls).

## Important documents

- [Changelog](CHANGELOG.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [License](https://raw.githubusercontent.com/ethjs/ethjs-provider-http/master/LICENSE)

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
