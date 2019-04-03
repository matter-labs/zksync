'use strict'
var Buffer = require('safe-buffer').Buffer
var BN = require('./bn')
var ECJPoint = require('./ecjpoint')

function ECPoint (x, y) {
  if (x === null && y === null) {
    this.x = this.y = null
    this.inf = true
  } else {
    this.x = x
    this.y = y
    this.inf = false
  }
}

ECPoint.fromPublicKey = function (publicKey) {
  var first = publicKey[0]
  var x
  var y

  if (publicKey.length === 33 && (first === 0x02 || first === 0x03)) {
    x = BN.fromBuffer(publicKey.slice(1, 33))

    // overflow
    if (x.ucmp(BN.p) >= 0) return null

    // create from X
    y = x.redSqr().redMul(x).redIAdd7().redSqrt()
    if (y === null) return null
    if ((first === 0x03) !== y.isOdd()) y = y.redNeg()

    return new ECPoint(x, y)
  }

  if (publicKey.length === 65 && (first === 0x04 || first === 0x06 || first === 0x07)) {
    x = BN.fromBuffer(publicKey.slice(1, 33))
    y = BN.fromBuffer(publicKey.slice(33, 65))

    // overflow
    if (x.ucmp(BN.p) >= 0 || y.ucmp(BN.p) >= 0) return null

    // is odd flag
    if ((first === 0x06 || first === 0x07) && y.isOdd() !== (first === 0x07)) return null

    // x*x*x + 7 = y*y
    if (x.redSqr().redMul(x).redIAdd7().ucmp(y.redSqr()) !== 0) return null

    return new ECPoint(x, y)
  }

  return null
}

ECPoint.prototype.toPublicKey = function (compressed) {
  var x = this.x
  var y = this.y
  var publicKey

  if (compressed) {
    publicKey = Buffer.alloc(33)
    publicKey[0] = y.isOdd() ? 0x03 : 0x02
    x.toBuffer().copy(publicKey, 1)
  } else {
    publicKey = Buffer.alloc(65)
    publicKey[0] = 0x04
    x.toBuffer().copy(publicKey, 1)
    y.toBuffer().copy(publicKey, 33)
  }

  return publicKey
}

ECPoint.fromECJPoint = function (p) {
  if (p.inf) return new ECPoint(null, null)

  var zinv = p.z.redInvm()
  var zinv2 = zinv.redSqr()
  var ax = p.x.redMul(zinv2)
  var ay = p.y.redMul(zinv2).redMul(zinv)

  return new ECPoint(ax, ay)
}

ECPoint.prototype.toECJPoint = function () {
  if (this.inf) return new ECJPoint(null, null, null)

  return new ECJPoint(this.x, this.y, ECJPoint.one)
}

ECPoint.prototype.neg = function () {
  if (this.inf) return this

  return new ECPoint(this.x, this.y.redNeg())
}

ECPoint.prototype.add = function (p) {
  // O + P = P
  if (this.inf) return p

  // P + O = P
  if (p.inf) return this

  if (this.x.ucmp(p.x) === 0) {
    // P + P = 2P
    if (this.y.ucmp(p.y) === 0) return this.dbl()
    // P + (-P) = O
    return new ECPoint(null, null)
  }

  // s = (y - yp) / (x - xp)
  // nx = s^2 - x - xp
  // ny = s * (x - nx) - y
  var s = this.y.redSub(p.y)
  if (!s.isZero()) s = s.redMul(this.x.redSub(p.x).redInvm())

  var nx = s.redSqr().redISub(this.x).redISub(p.x)
  var ny = s.redMul(this.x.redSub(nx)).redISub(this.y)
  return new ECPoint(nx, ny)
}

ECPoint.prototype.dbl = function () {
  if (this.inf) return this

  // 2P = O
  var yy = this.y.redAdd(this.y)
  if (yy.isZero()) return new ECPoint(null, null)

  // s = (3 * x^2) / (2 * y)
  // nx = s^2 - 2*x
  // ny = s * (x - nx) - y
  var x2 = this.x.redSqr()
  var s = x2.redAdd(x2).redIAdd(x2).redMul(yy.redInvm())

  var nx = s.redSqr().redISub(this.x.redAdd(this.x))
  var ny = s.redMul(this.x.redSub(nx)).redISub(this.y)
  return new ECPoint(nx, ny)
}

ECPoint.prototype.mul = function (num) {
  // Algorithm 3.36 Window NAF method for point multiplication
  var nafPoints = this._getNAFPoints(4)
  var points = nafPoints.points

  // Get NAF form
  var naf = num.getNAF(nafPoints.wnd)

  // Add `this`*(N+1) for every w-NAF index
  var acc = new ECJPoint(null, null, null)
  for (var i = naf.length - 1; i >= 0; i--) {
    // Count zeroes
    for (var k = 0; i >= 0 && naf[i] === 0; i--, ++k);
    if (i >= 0) k += 1
    acc = acc.dblp(k)

    if (i < 0) break

    // J +- P
    var z = naf[i]
    if (z > 0) {
      acc = acc.mixedAdd(points[(z - 1) >> 1])
    } else {
      acc = acc.mixedAdd(points[(-z - 1) >> 1].neg())
    }
  }

  return ECPoint.fromECJPoint(acc)
}

ECPoint.prototype._getNAFPoints1 = function () {
  return { wnd: 1, points: [this] }
}

ECPoint.prototype._getNAFPoints = function (wnd) {
  var points = new Array((1 << wnd) - 1)
  points[0] = this
  var dbl = this.dbl()
  for (var i = 1; i < points.length; ++i) points[i] = points[i - 1].add(dbl)
  return { wnd: wnd, points: points }
}

module.exports = ECPoint
