'use strict'
var createHmac = require('create-hmac')
var hashInfo = require('./lib/hash-info.json')

var ebuf = new Buffer(0)
var b0x00 = new Buffer([ 0x00 ])
var b0x01 = new Buffer([ 0x01 ])

function HmacDRBG (algo, entropy, nonce, pers) {
  var info = hashInfo[algo]
  if (info === undefined) throw new Error('hash ' + algo + ' is not supported')

  this._algo = algo
  this._securityStrength = info.securityStrength / 8
  this._outlen = info.outlen / 8
  this._reseedInterval = 0x1000000000000 // 2**48

  this._init(entropy, nonce, pers)
}

HmacDRBG.prototype._update = function (seed) {
  var kmac = createHmac(this._algo, this._K).update(this._V).update(b0x00)
  if (seed) kmac.update(seed)

  this._K = kmac.digest()
  this._V = createHmac(this._algo, this._K).update(this._V).digest()
  if (!seed) return

  this._K = createHmac(this._algo, this._K).update(this._V).update(b0x01).update(seed).digest()
  this._V = createHmac(this._algo, this._K).update(this._V).digest()
}

HmacDRBG.prototype._init = function (entropy, nonce, pers) {
  if (entropy.length < this._securityStrength) throw new Error('Not enough entropy')

  this._K = new Buffer(this._outlen)
  this._V = new Buffer(this._outlen)
  for (var i = 0; i < this._K.length; ++i) {
    this._K[i] = 0x00
    this._V[i] = 0x01
  }

  this._update(Buffer.concat([ entropy, nonce, pers || ebuf ]))
  this._reseed = 1
}

HmacDRBG.prototype.reseed = function (entropy, add) {
  if (entropy.length < this._securityStrength) throw new Error('Not enough entropy')

  this._update(Buffer.concat([ entropy, add || ebuf ]))
  this._reseed = 1
}

HmacDRBG.prototype.generate = function (len, add) {
  if (this._reseed > this._reseedInterval) throw new Error('Reseed is required')

  if (add && add.length === 0) add = undefined
  if (add) this._update(add)

  var temp = new Buffer(0)
  while (temp.length < len) {
    this._V = createHmac(this._algo, this._K).update(this._V).digest()
    temp = Buffer.concat([ temp, this._V ])
  }

  this._update(add)
  this._reseed += 1
  return temp.slice(0, len)
}

module.exports = HmacDRBG
