'use strict'
var createHash = require('create-hash')
var hashInfo = require('./lib/hash-info.json')
var util = require('./lib/util')

var ebuf = new Buffer(0)
var b0x00 = new Buffer([ 0x00 ])
var b0x01 = new Buffer([ 0x01 ])
var b0x02 = new Buffer([ 0x02 ])
var b0x03 = new Buffer([ 0x03 ])

function HashDRBG (algo, entropy, nonce, pers) {
  var info = hashInfo[algo]
  if (info === undefined) throw new Error('hash ' + algo + ' is not supported')

  this._algo = algo
  this._securityStrength = info.securityStrength / 8
  this._outlen = info.outlen / 8
  this._seedlen = info.seedlen / 8
  this._reseedInterval = 0x1000000000000 // 2**48

  this._init(entropy, nonce, pers)
}

HashDRBG.prototype._hashdf = function (input, len) {
  var data = Buffer.concat([
    new Buffer([ 0x01 ]),
    new Buffer([ (len >>> 21) & 0xff, (len >>> 13) & 0xff, (len >>> 5) & 0xff, (len & 0x1f) << 3 ]),
    input
  ])

  var temp = new Buffer(0)
  for (var i = 1, m = Math.ceil(len / this._outlen); i <= m; ++i) {
    temp = Buffer.concat([ temp, createHash(this._algo).update(data).digest() ])
    data[0] += 1
  }

  return temp.slice(0, len)
}

HashDRBG.prototype._init = function (entropy, nonce, pers) {
  if (entropy.length < this._securityStrength) throw new Error('Not enough entropy')

  var seedMaterial = Buffer.concat([ entropy, nonce, pers || ebuf ])
  this._V = this._hashdf(seedMaterial, this._seedlen)
  this._C = this._hashdf(Buffer.concat([ b0x00, this._V ]), this._seedlen)
  this._reseed = 1
}

HashDRBG.prototype.reseed = function (entropy, add) {
  if (entropy.length < this._securityStrength) throw new Error('Not enough entropy')

  var seedMaterial = Buffer.concat([ b0x01, this._V, entropy, add || ebuf ])
  this._V = this._hashdf(seedMaterial, this._seedlen)
  this._C = this._hashdf(Buffer.concat([ b0x00, this._V ]), this._seedlen)
  this._reseed = 1
}

HashDRBG.prototype.generate = function (len, add) {
  if (this._reseed > this._reseedInterval) throw new Error('Reseed is required')

  if (add && add.length !== 0) {
    var data = Buffer.concat([ b0x02, this._V, add ])
    var w = createHash(this._algo).update(data).digest()
    this._V = util.bsum([ this._V, w ])
  }

  var result = this._hashgen(len)

  var H = createHash(this._algo).update(Buffer.concat([ b0x03, this._V ])).digest()
  var bReseed = new Buffer(8)
  bReseed.writeUInt32BE((this._reseed / 0x0100000000) | 0, 0)
  bReseed.writeUInt32BE(this._reseed >>> 0, 4)
  this._V = util.bsum([ this._V, H, this._C, bReseed ])

  this._reseed += 1
  return result
}

HashDRBG.prototype._hashgen = function (len) {
  var data = new Buffer(this._V)
  var W = new Buffer(0)
  while (W.length < len) {
    var w = createHash(this._algo).update(data).digest()
    W = Buffer.concat([ W, w ])
    data = util.bsum([ data, b0x01 ])
  }

  return W.slice(0, len)
}

module.exports = HashDRBG
