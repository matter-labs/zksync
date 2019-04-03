SYNOPSIS
=====
[![NPM Package](https://img.shields.io/npm/v/rlp.svg?style=flat-square)](https://www.npmjs.org/package/rlp)
[![Build Status](https://img.shields.io/travis/ethereumjs/rlp.svg?branch=master&style=flat-square)](https://travis-ci.org/ethereumjs/rlp)
[![Coverage Status](https://img.shields.io/coveralls/ethereumjs/rlp.svg?style=flat-square)](https://coveralls.io/r/ethereumjs/rlp)
[![Gitter](https://img.shields.io/gitter/room/ethereum/ethereumjs-lib.svg?style=flat-square)](https://gitter.im/ethereum/ethereumjs-lib) or #ethereumjs on freenode

[![js-standard-style](https://cdn.rawgit.com/feross/standard/master/badge.svg)](https://github.com/feross/standard)


[Recursive Length](https://github.com/ethereum/wiki/wiki/RLP) Prefix Encoding for node.js.

INSTALL
======
`npm install rlp`   

install with `-g` if you want to use the cli.

USAGE
=======

```javascript
var RLP = require('rlp');
var assert = require('assert');

var nestedList = [ [], [[]], [ [], [[]] ] ];
var encoded = RLP.encode(nestedList);
var decoded = RLP.decode(encoded);
assert.deepEqual(nestedList, decoded);


```

API
=====
`rlp.encode(plain)` - RLP encodes an `Array`, `Buffer` or `String` and returns a `Buffer`.

`rlp.decode(encoded, [skipRemainderCheck=false])` - Decodes an RLP encoded `Buffer`, `Array` or `String` and returns a `Buffer` or an `Array` of `Buffers`. If `skipRemainderCheck` is enabled, `rlp` will just decode the first rlp sequence in the buffer. By default, it would throw an error if there are more bytes in Buffer than used by rlp sequence.

CLI
===
`rlp decode <hex string>`   
`rlp encode <json String>`  

TESTS
=====
Test uses mocha. To run `npm test`

CODE COVERAGE
=============
Install dev dependencies
`npm install`

Run
`npm run coverage`

The results are at
`coverage/lcov-report/index.html`
