# Scrypt For Node

[![Build Status](https://travis-ci.org/barrysteyn/node-scrypt.png?branch=master)](https://travis-ci.org/barrysteyn/node-scrypt)
[![npm version](https://badge.fury.io/js/scrypt.svg)](http://badge.fury.io/js/scrypt)

Scrypt for Node/IO is a native node/io C++ wrapper for Colin Percival's
[scrypt](https://www.tarsnap.com/scrypt.html) cryptographic hash utility.

As should be the case with any security tool, this library should be scrutinized
by anyone using it. If you find or suspect an issue with the code- please bring
it to my attention and I'll spend some time trying to make sure that this tool is
as secure as possible.

## Node-Scrypt Version 6
Version 6 is a major new release. It is by and large compatible with version 5.
 
  * Scrypt version 1.2.0 is being used (a very recently released version of Scrypt)
  * Using Node's internal cryptographic libraries - for windows users, there is no need to use an external OpenSSL library anymore.
  * Using Node's OS module to check for freemem, meaning no need to use any system calls and therefore no external dependencies

Version 6 should work much better on all platforms

## Past Releases
### Node-Scrypt Version 5
Version 5 is a major new release that is **not backward compatible** with any
previous version. Some highlights:

  * C++ addon code rewritten:
    * Using [Nan 2.x](https://github.com/nodejs/nan)
    * Code has been greatly simplified
  * ES6 Promise aware.
  * API has changed:
    * Every output is a buffer.
    * Separated functions into async and sync versions.
    * Api name swap: What was kdf in previous versions is now hash (and vice versa).
    * Async functions will return a Promise if no callback function is present and Promises are available (else it will throw a SyntaxError).
  * Using correct [JavaScript Error](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error) object for all errors.

### Migrating To Version 5
Version 5 is not backward compatible, but it should still be easy to migrate.
Please read the [api section](#api) to see what's changed. One big change that is
worth noting is a name change: What used to be called **hash** has now been
changed to **kdf** and conversely, what was **kdf** is now called **hash**.

## Table Of Contents

 * [Scrypt](#scrypt)
 * [Installation Instructions](#installation-instructions)
 * [API](#api) - The module consists of four functions:
   * [params](#params) - a translation function that produces scrypt parameters
   * [kdf](#kdf) - a key derivation function designed for password hashing
   * [verifyKdf](#verifykdf) - checks if a key matches a kdf
   * [hash](#hash) - the raw underlying scrypt hash function
 * [Example Usage](#example-usage)
 * [FAQ](#faq)
 * [Roadmap and Changelog](#roadmap)
 * [Credits](#credits)

# Scrypt
Scrypt is an advanced crypto library used mainly for [key derivation](http://en.wikipedia.org/wiki/Key_derivation_function):
More information can be found here:

* [Tarsnap blurb about scrypt](http://www.tarsnap.com/scrypt.html) - Colin Percival
(the author of scrypt) explains a bit about it.
* [Academic paper explaining scrypt](http://www.tarsnap.com/scrypt/scrypt.pdf).
* [Wikipedia Article on scrypt](http://en.wikipedia.org/wiki/Scrypt).

# Installation Instructions

## Pre-Requisistes
### Windows

 * [Node-Gyp](https://github.com/TooTallNate/node-gyp) for Windows:
   * Installation instructions: [node-gyp for windows](https://github.com/TooTallNate/node-gyp#installation)
   * Look [here](https://github.com/TooTallNate/node-gyp/wiki/Visual-Studio-2010-Setup) for additional information/helpful hints.

### Linux/MacOS
[Node-gyp](https://github.com/TooTallNate/node-gyp) is needed to build this module. It should be installed globally, that is, with the `-g` switch:

    npm install -g node-gyp

## Install From NPM

    npm install scrypt

## Install From Source

    git clone https://github.com/barrysteyn/node-scrypt.git
    cd node-scrypt
    npm install
    node-gyp configure build

## Testing
To test, go to the folder where scrypt was installed, and type:

    npm test

# API

## params
Translates human understandable parameters to scrypt's internal parameters.

>
  scrypt.paramsSync <br>
  scrypt.params(maxtime, [maxmem, [max_memfrac]], [function(err, obj) {}])

  * maxtime - [REQUIRED] - a decimal (double) representing the maximum amount of time in seconds scrypt will spend when computing the derived key.
  * maxmem - [OPTIONAL] - an integer, specifying the maximum number of bytes of RAM used when computing the derived encryption key. If not present, will default to 0.
  * maxmemfrac - [OPTIONAL only if maxmem is present] - a double value between 0.0 and 1.0, representing the fraction (normalized percentage value) of the available RAM used when computing the derived key. If not present, will default to 0.5.
  * callback_function - [OPTIONAL] - not applicable to synchronous function. If present in async function, then it will be treated as a normal async callback. If not present, a Promise will be returned if ES6 promises are available. If not present and ES6 promises are not present, a SyntaxError will be thrown.

## kdf
**Note**: In previous versions, this was called *hash*.

Produces a key derivation function that uses the scrypt hash function. This
should be used for hashing and checking passwords as it incorporates salt as well
as [HMAC](https://en.wikipedia.org/wiki/Hash-based_message_authentication_code) into
its format. It is based on a design by Colin Percival, the author of scrypt. The format
can be seen [here](http://security.stackexchange.com/questions/88678/why-does-node-js-scrypt-function-use-hmac-this-way/91050#91050).

>
  scrypt.kdfSync <br>
  scrypt.kdf(key, paramsObject, [function(err, obj){}])

  * key - [REQUIRED] - a string (or buffer) representing the key (password) that is to be hashed.
  * paramsObject - [REQUIRED] - parameters to control scrypt hashing (see params above).
  * callback_function - [OPTIONAL] - not applicable to synchronous function. If present in async function, then it will be treated as a normal async callback. If not present, a Promise will be returned if ES6 promises are available. If not present and ES6 promises are not present, a SyntaxError will be thrown.

## verifyKdf

Checks if a key (password) matches a kdf.

>
  scrypt.verifyKdfSync <br>
  scrypt.verifyKdf(kdf, key, [function(err, result){}])

 * kdf [REQUIRED] - see kdf above.
 * key - [REQUIRED] - a string (or buffer) representing the key (password) that is to be checked.
 * callback_function - [OPTIONAL] - not applicable to synchronous function. If present in async function, then it will be treated as a normal async callback. If not present, a Promise will be returned if ES6 promises are available. If not present and ES6 promises are not present, a SyntaxError will be thrown.

## hash
**Note**: In previous versions, this was called *kdf*.

This is the raw scrypt hash function.

>
  scrypt.hashSync <br>
  scrypt.hash(key, paramsObject, output_length, salt, function(err, obj){})

  * key - [REQUIRED] - a string (or buffer) representing the key (password) that is to be checked.
  * paramsObject - [REQUIRED] - parameters to control scrypt hashing (see params above).
  * output_length - [REQUIRED] - the length of the resulting hashed output.
  * salt - [REQUIRED] - a string (or buffer) used for salt. The string (or buffer) can be empty.
  * callback_function - [OPTIONAL] - not applicable to synchronous function. If present in async function, then it will be treated as a normal async callback. If not present, a Promise will be returned if ES6 promises are available. If not present and ES6 promises are not present, a SyntaxError will be thrown.

# Example Usage

## params

```JavaScript
var scrypt = require("scrypt");

//Synchronous
try {
  //Uses 0.1 for maxtime, and default values maxmem and maxmemfrac
  var scryptParameters = scrypt.paramsSync(0.1);
  console.log(scryptParameters);
} catch(err) {
  //handle error
}

//Asynchronous with callback
scrypt.params(0.1, function(err, scryptParameters) {
  console.log(scryptParameters);
});

//Asynchronous with promise
scrypt.params(0.1).then(function(result){
  console.log(result);
}, function(err) {
  console.log(err);
});
```

## kdf

```JavaScript
var scrypt = require("scrypt");
var scryptParameters = scrypt.paramsSync(0.1);
var key = new Buffer("this is a key"); //could also be a string

//Synchronous example that will output in hexidecimal encoding
var kdfResult = scrypt.kdfSync(key, scryptParameters); //should be wrapped in try catch, but leaving it out for brevity
console.log("Synchronous result: "+kdfResult.toString("hex"));

//Asynchronous example that expects key to be ascii encoded
scrypt.kdf("ascii encoded key", {N: 1, r:1, p:1}, function(err, result){
  //Note how scrypt parameters was passed as a JSON object
  console.log("Asynchronous result: "+result.toString("base64"));
});

//Asynchronous with promise
scrypt.kdf("ascii encoded key", {N: 1, r:1, p:1}).then(function(result){
  console.log("Asynchronous result: "+result.toString("base64"));
}, function(err){
});
```

## verifyKdf

```JavaScript
var scrypt = require("scrypt");
var scryptParameters = scrypt.paramsSync(0.1);
var kdfResult = scrypt.kdfSync("password", scryptParameters);

//Synchronous
scrypt.verifyKdfSync(kdfResult, "password"); // returns true
scrypt.verifyKdfSync(kdfResult, "incorrect password"); // returns false

//Asynchronous
scrypt.verifyKdf(kdfResult, new Buffer("password"), function(err, result) {
  //result will be true
});

//Asynchronous with promise
scrypt.verifyKdf(kdfResult, "incorrect password").then(function(result) {
  //result will be false
}, function(err) {
});
```

## hash
The [scrypt paper](http://www.tarsnap.com/scrypt/scrypt.pdf) lists four [test vectors](http://tools.ietf.org/html/draft-josefsson-scrypt-kdf-00#page-11) to test implementation. This example will show how to produce these test vectors from within this module.

### Test Vector 1

```JavaScript
var scrypt = require("scrypt");
var key = new Buffer("");

//Synchronous
var result = scrypt.hashSync(key,{"N":16,"r":1,"p":1},64,"");
console.log(result.toString("hex"));

//Asynchronous
scrypt.hash(key, {"N":16,"r":1,"p":1},64,"", function(err, res) {
  console.log(result.toString("hex"));
});

//Asynchronous with promise
scrypt.hash(key, {"N":16,"r":1,"p":1},64,"").then(function(result) {
  console.log(result.toString("hex"));
}, function(err){});
```

### Test Vector 2

```JavaScript
var scrypt = require("scrypt");
var salt = new Buffer("NaCl");

//Synchronous
var result = scrypt.hashSync("password", {"N":1024,"r":8,"p":16}, 64, salt);
console.log(result.toString("hex"));

scrypt.hash("password", {"N":1024,"r":8,"p":16},64,salt, function(err, result) {
  console.log(result.toString("hex"));
});
```

### Test Vector 3

```JavaScript
var scrypt = require("scrypt");
var key = new Buffer("pleaseletmein");
var salt = new Buffer("SodiumChloride");

//Synchronous
var result = scrypt.hashSync(key,{"N":16384,"r":8,"p":1},64,salt);
console.log(result.toString("hex"));

//Asynchronous
scrypt.hash(key, {"N":16384,"r":8,"p":1}, 64, salt, function(err, result) {
  console.log(result.toString("hex"));
});
```

### Test Vector 4
Note: This test vector is very taxing in terms of resources.

```JavaScript
var scrypt = require("scrypt");

//Synchronous
var result = scrypt.hashSync("pleaseletmein",{"N":1048576,"r":8,"p":1},64,"SodiumChloride");
console.log(result.toString("hex"));

//Asynchronous
scrypt.hash("pleaseletmein", {"N":1048576,"r":8,"p":1},64,"SodiumChloride", function(err, result) {
  console.log(result.toString("hex"));
});
```

# FAQ
## General
### What Platforms Are Supported?
This module supports most posix platforms, as well as Microsoft Windows. It has been tested on the
following platforms: **Linux**, **MAC OS**, **SmartOS** (so its ready for Joyent Cloud)
and **Microsoft Windows**. It also works on FreeBSD, OpenBSD, SunOS etc.

## Scrypt
### Why Use Scrypt?

It is probably the most advanced key derivation function available. This is is quote taken
from a comment in hacker news:

>Passwords hashed with scrypt with sufficiently-high strength values (there are 3 tweakable
input numbers) are fundamentally impervious to being cracked. I use the word "fundamental"
in the literal sense, here; even if you had the resources of a large country, you would not
be able to design any hardware (whether it be GPU hardware, custom-designed hardware, or
otherwise) which could crack these hashes. Ever. (For sufficiently-small definitions of
"ever". At the very least "within your lifetime"; probably far longer.)

### What Are The Pros And Cons For Using Scrypt?
#### Pros

* The scrypt algorithm has been published by [IETF](http://en.wikipedia.org/wiki/IETF)
as an [Internet Draft](http://en.wikipedia.org/wiki/Internet_Draft) and is thus on track to becoming a standard. See [here](https://tools.ietf.org/html/draft-josefsson-scrypt-kdf-00) for the draft.
* It is being actively used in production at [Tarsnap](http://www.tarsnap.com/).
* It is much more secure than bcrypt.
* It is designed to be future proof against attacks with future (and more advanced) hardware.
* It is designed to defend against large scale custom hardware attacks.
* It is production ready.
* There is a scrypt library for most major scripting languages
(Python, Ruby etc). Now this module provides the library for NodeJS :)

I will end this section with a quote from Colin Percival (author of scrypt):

> We estimate that on modern (2009) hardware, if 5 seconds are spent computing a derived key,
the cost of a hardware brute-force attack against scrypt is roughly 4000 times greater than the
cost of a similar attack against bcrypt (to find the same password), and 20000 times greater
than a similar attack against PBKDF2.

#### Cons
There is just one con I can think of: It is a relatively new library (only been around since 2009).
Cryptographers don't really like new libraries for production deployment as it has not been *battle
tested*. That being said, it is being actively used in [Tarsnap](http://www.tarsnap.com/)
(as mentioned above) and the author is very active.

## Using Scrypt With Passwords
### What Are The Essential Properties For Storing Passwords?
Storing passwords requires three essential properties

* The password must not be stored in plaintext.
* The password hash must be salted. (Making a rainbow table attack very
difficult to pull off).
* The salted hash function must not be fast. (If someone does get hold
of the salted hashes, their only option will be brute force which will
be very slow).

As an example of how storing passwords can be done badly, take [LinkedIn](http://www.linkedin.com).
In 2012, they [came under fire](http://thenextweb.com/socialmedia/2012/06/06/bad-day-for-linkedin-6-5-million-hashed-passwords-reportedly-leaked-change-yours-now/#!rS1HT)
for using unsalted hashes to store their passwords. As most commentators at
the time were focusing no salt being present, the big picture was missed.
In fact, their biggest problem was that they used [sha1](http://en.wikipedia.org/wiki/SHA-1),
a very fast hash function.

### If random salts are used, why do all resulting KDF's start with *c2NyeXB0*?
The kdf has a [specific format](http://security.stackexchange.com/questions/88678/why-does-node-js-scrypt-function-use-hmac-this-way/91050#91050):
The word *"scrypt"* is added as a prefix. The reason for this is because
I am sticking to Colin Percival's (the creator of scrypt) reference implementation,
whereby he prefixes *scrypt* in this way. The base64 encoding of the ascii *"scrypt"*
is *c2NyeXB0*. The scrypt parameters are then appended. Users of scrypt normally do
not change this information once it is settled upon (hence this will also look the
be identical).


To illustrate with an example, I have hashed two password: *password1* and *password2*.
Their Base64 outputs are as follows:

    password1
    c2NyeXB0AAwAAAAIAAAAAcQ0zwp7QNLklxCn14vB75AYWDIrrT9I/7F9+lVGBfKN/1TH2hs
    /HboSy1ptzN0YzHJhC7PZIEPQzf2nuoaqVZg8VkKEJlo8/QaH7qjU2VwB

    password2
    c2NyeXB0AAwAAAAIAAAAAZ/+bp8gWcTZgEC7YQZeLLyxFeKRRdDkwbaGeFC0NkdUr/YFAWY
    /UwdOH4i/PxW48fXeXBDOTvGWtS3lLUgzNM0PlJbXhMOGd2bke0PvTSnW

As one can see from the above example, both hashes start off by looking similar (they both start
with *c2NyeXB0AAwAAAAIAAAAA* - as explained above), but after this, things change very rapidly.
In fact, I hashed the password *password1* again:

    password1
    c2NyeXB0AAwAAAAIAAAAATpP+fdQAryDiRmCmcoOrZa2mZ049KdbA/ofTTrATQQ+m
    0L/gR811d0WQyip6p2skXVEMz2+8U+xGryFu2p0yzfCxYLUrAaIzaZELkN2M6k0

Compare this hash to the one above. Even though they start off looking similar, their outputs
are vastly different (even though it is the same password being hashed). This is because of
the **random** salt that has been added, ensuring that no two hashes will ever be identical,
even if the password that is being hashed is the same.

For those that are curious or paranoid, please look at how the kdf is both [produced](https://github.com/barrysteyn/node-scrypt/blob/master/src/scryptwrapper/keyderivation.c#L36-L80)
and [verified](https://github.com/barrysteyn/node-scrypt/blob/master/src/scryptwrapper/keyderivation.c#L82-L121) (you are going to need some knowledge of the [C language](http://c.learncodethehardway.org/book/) for this).

# Roadmap
See [changelog](https://github.com/barrysteyn/node-scrypt/blob/master/changelog.md) for upcoming features.

# Credits
The scrypt library is Colin Percival's [scrypt](http://www.tarsnap.com/scrypt.html) project.

Syed Beparey was instrumental in getting the Windows build working, with most of the Windows build based off the work done by Dinesh Shanbhag.
