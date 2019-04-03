# A Node.js C++ extension for SHA-3 (Keccak)

[![Travis CI][3]][4]
[![npm version][5]][6]
[![npm downloads][7]][6]
[![dependencies][8]][9]
[![devDependencies][10]][9]
[![license][11]][12]

This Node.js extension implements the SHA-3 ([Keccak][1]) cryptographic hashing algorithm. It is based on the reference C implementation, version 3.2. The exposed interface is almost identical to that of the `crypto` standard library.

[![Phusion][13]][2]

## Installation

Via `npm`:

```bash
$ npm install sha3
```

Via `yarn`:

```bash
$ yarn add sha3
```

## Usage

Keccak supports 5 hash lengths: 224-bit, 256-bit, 384-bit, 512-bit and variable length. Variable length is not supported by this Node.js extension. Unless the user specifies otherwise, this Node.js extension assumes 512-bit.

```javascript
const SHA3 = require('sha3');

// Generate 512-bit digest.
let d = new SHA3.SHA3Hash();
d.update('foo');
d.digest('hex');
// => "1597842a..."

// Generate 224-bit digest.
d = new SHA3.SHA3Hash(224);
d.update('foo');
d.digest('hex');
// => "daa94da7..."
```

### new SHA3Hash([hashlen])

This is the hash object. `hashlen` is 512 by default.

### hash.update(data, [input_encoding])

Updates the hash content with the given data, the encoding of which is given in `input_encoding` and can be `'utf8'`, `'ascii'` or `'binary'`. Defaults to `'binary'`. This can be called many times with new data as it is streamed.

### hash.digest([encoding])

Calculates the digest of all of the passed data to be hashed. The encoding can be `'hex'` or `'binary'`. Defaults to `'binary'`.

Note: unlike `crypto.Hash`, a `SHA3Hash` object _can_ still be used after the `digest()` method been called.

## Running the test suite

Run the test suite as follows:

```bash
$ npm test
```

The test suite is automatically generated from Keccak's reference test suite.
It requires that you have Python 2.7 installed and available via the
`python` executable.

## Warning

Do not use SHA-3 for hashing passwords. Do not even use SHA-3 + salt for hashing passwords. Use a [slow hash][14] instead.

## See also

[Digest::SHA3 for Ruby](https://github.com/phusion/digest-sha3-ruby)

[1]: https://keccak.team/keccak.html
[2]: https://www.phusion.nl/
[3]: https://img.shields.io/travis/phusion/node-sha3/master.svg?label=Travis%20CI
[4]: https://travis-ci.org/phusion/node-sha3
[5]: https://img.shields.io/npm/v/sha3.svg
[6]: https://www.npmjs.com/package/sha3
[7]: https://img.shields.io/npm/dt/sha3.svg
[8]: https://img.shields.io/david/phusion/node-sha3.svg
[9]: https://github.com/phusion/node-sha3/blob/master/package.json
[10]: https://img.shields.io/david/dev/phusion/node-sha3.svg
[11]: https://img.shields.io/github/license/phusion/node-sha3.svg
[12]: https://github.com/phusion/node-sha3/blob/master/LICENSE
[13]: https://www.phusion.nl/images/header/pinwheel_logo.svg
[14]: http://codahale.com/how-to-safely-store-a-password/
