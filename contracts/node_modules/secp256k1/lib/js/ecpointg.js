'use strict'
var Buffer = require('safe-buffer').Buffer
var BN = require('./bn')
var ECPoint = require('./ecpoint')
var ECJPoint = require('./ecjpoint')

function ECPointG () {
  this.x = BN.fromBuffer(Buffer.from('79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798', 'hex'))
  this.y = BN.fromBuffer(Buffer.from('483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8', 'hex'))
  this.inf = false

  this._precompute()
}

ECPointG.prototype._precompute = function () {
  var ecpoint = new ECPoint(this.x, this.y)

  var dstep = 4
  var points = new Array(1 + Math.ceil(257 / dstep))
  var acc = points[0] = ecpoint
  for (var i = 1; i < points.length; ++i) {
    for (var j = 0; j < dstep; j++) acc = acc.dbl()
    points[i] = acc
  }

  this.precomputed = {
    naf: ecpoint._getNAFPoints(7),
    doubles: {
      step: dstep,
      points: points,
      negpoints: points.map(function (p) { return p.neg() })
    }
  }
}

ECPointG.prototype.mul = function (num) {
  // Algorithm 3.42 Fixed-base NAF windowing method for point multiplication
  var step = this.precomputed.doubles.step
  var points = this.precomputed.doubles.points
  var negpoints = this.precomputed.doubles.negpoints

  var naf = num.getNAF(1)
  var I = ((1 << (step + 1)) - (step % 2 === 0 ? 2 : 1)) / 3

  // Translate into more windowed form
  var repr = []
  for (var j = 0; j < naf.length; j += step) {
    var nafW = 0
    for (var k = j + step - 1; k >= j; k--) nafW = (nafW << 1) + naf[k]
    repr.push(nafW)
  }

  var a = new ECJPoint(null, null, null)
  var b = new ECJPoint(null, null, null)
  for (var i = I; i > 0; i--) {
    for (var jj = 0; jj < repr.length; jj++) {
      if (repr[jj] === i) {
        b = b.mixedAdd(points[jj])
      } else if (repr[jj] === -i) {
        b = b.mixedAdd(negpoints[jj])
      }
    }

    a = a.add(b)
  }

  return ECPoint.fromECJPoint(a)
}

ECPointG.prototype.mulAdd = function (k1, p2, k2) {
  var nafPointsP1 = this.precomputed.naf
  var nafPointsP2 = p2._getNAFPoints1()
  var wnd = [nafPointsP1.points, nafPointsP2.points]
  var naf = [k1.getNAF(nafPointsP1.wnd), k2.getNAF(nafPointsP2.wnd)]

  var acc = new ECJPoint(null, null, null)
  var tmp = [null, null]
  for (var i = Math.max(naf[0].length, naf[1].length); i >= 0; i--) {
    var k = 0

    for (; i >= 0; ++k, --i) {
      tmp[0] = naf[0][i] | 0
      tmp[1] = naf[1][i] | 0

      if (tmp[0] !== 0 || tmp[1] !== 0) break
    }

    if (i >= 0) k += 1
    acc = acc.dblp(k)

    if (i < 0) break

    for (var jj = 0; jj < 2; jj++) {
      var z = tmp[jj]
      var p
      if (z === 0) {
        continue
      } else if (z > 0) {
        p = wnd[jj][z >> 1]
      } else if (z < 0) {
        p = wnd[jj][-z >> 1].neg()
      }

      // hack: ECPoint detection
      if (p.z === undefined) {
        acc = acc.mixedAdd(p)
      } else {
        acc = acc.add(p)
      }
    }
  }

  return acc
}

module.exports = new ECPointG()
