'use strict'
var BN = require('./bn')

function ECJPoint (x, y, z) {
  if (x === null && y === null && z === null) {
    this.x = ECJPoint.one
    this.y = ECJPoint.one
    this.z = ECJPoint.zero
  } else {
    this.x = x
    this.y = y
    this.z = z
  }

  this.zOne = this.z === ECJPoint.one
}

ECJPoint.zero = BN.fromNumber(0)
ECJPoint.one = BN.fromNumber(1)

ECJPoint.prototype.neg = function () {
  if (this.inf) return this

  return new ECJPoint(this.x, this.y.redNeg(), this.z)
}

ECJPoint.prototype.add = function (p) {
  // O + P = P
  if (this.inf) return p

  // P + O = P
  if (p.inf) return this

  // http://hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#addition-add-1998-cmo-2
  // 12M + 4S + 7A
  var pz2 = p.z.redSqr()
  var z2 = this.z.redSqr()
  var u1 = this.x.redMul(pz2)
  var u2 = p.x.redMul(z2)
  var s1 = this.y.redMul(pz2).redMul(p.z)
  var s2 = p.y.redMul(z2).redMul(this.z)

  var h = u1.redSub(u2)
  var r = s1.redSub(s2)
  if (h.isZero()) {
    if (r.isZero()) return this.dbl()
    return new ECJPoint(null, null, null)
  }

  var h2 = h.redSqr()
  var v = u1.redMul(h2)
  var h3 = h2.redMul(h)

  var nx = r.redSqr().redIAdd(h3).redISub(v).redISub(v)
  var ny = r.redMul(v.redISub(nx)).redISub(s1.redMul(h3))
  var nz = this.z.redMul(p.z).redMul(h)

  return new ECJPoint(nx, ny, nz)
}

ECJPoint.prototype.mixedAdd = function (p) {
  // O + P = P
  if (this.inf) return p.toECJPoint()

  // P + O = P
  if (p.inf) return this

  // http://hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#addition-add-1998-cmo-2
  //   with p.z = 1
  // 8M + 3S + 7A
  var z2 = this.z.redSqr()
  var u1 = this.x
  var u2 = p.x.redMul(z2)
  var s1 = this.y
  var s2 = p.y.redMul(z2).redMul(this.z)

  var h = u1.redSub(u2)
  var r = s1.redSub(s2)
  if (h.isZero()) {
    if (r.isZero()) return this.dbl()
    return new ECJPoint(null, null, null)
  }

  var h2 = h.redSqr()
  var v = u1.redMul(h2)
  var h3 = h2.redMul(h)

  var nx = r.redSqr().redIAdd(h3).redISub(v).redISub(v)
  var ny = r.redMul(v.redISub(nx)).redISub(s1.redMul(h3))
  var nz = this.z.redMul(h)

  return new ECJPoint(nx, ny, nz)
}

ECJPoint.prototype.dbl = function () {
  if (this.inf) return this

  var nx
  var ny
  var nz

  // Z = 1
  if (this.zOne) {
    // http://hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#doubling-mdbl-2007-bl
    // 1M + 5S + 6A + 3*2 + 1*3 + 1*8

    // XX = X1^2
    var xx = this.x.redSqr()
    // YY = Y1^2
    var yy = this.y.redSqr()
    // YYYY = YY^2
    var yyyy = yy.redSqr()
    // S = 2 * ((X1 + YY)^2 - XX - YYYY)
    var s = this.x.redAdd(yy).redSqr().redISub(xx).redISub(yyyy)
    s = s.redIAdd(s)
    // M = 3 * XX
    var m = xx.redAdd(xx).redIAdd(xx)
    // T = M ^ 2 - 2*S
    var t = m.redSqr().redISub(s).redISub(s)

    // 8 * YYYY
    var yyyy8 = yyyy.redIAdd(yyyy).redIAdd(yyyy).redIAdd(yyyy)

    // X3 = T
    nx = t
    // Y3 = M * (S - T) - 8 * YYYY
    ny = m.redMul(s.redISub(t)).redISub(yyyy8)
    // Z3 = 2*Y1
    nz = this.y.redAdd(this.y)
  } else {
    // http://hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#doubling-dbl-2009-l
    // 2M + 5S + 6A + 3*2 + 1*3 + 1*8

    // A = X1^2
    var a = this.x.redSqr()
    // B = Y1^2
    var b = this.y.redSqr()
    // C = B^2
    var c = b.redSqr()
    // D = 2 * ((X1 + B)^2 - A - C)
    var d = this.x.redAdd(b).redSqr().redISub(a).redISub(c)
    d = d.redIAdd(d)
    // E = 3 * A
    var e = a.redAdd(a).redIAdd(a)
    // F = E^2
    var f = e.redSqr()

    // 8 * C
    var c8 = c.redIAdd(c).redIAdd(c).redIAdd(c)

    // X3 = F - 2 * D
    nx = f.redISub(d).redISub(d)
    // Y3 = E * (D - X3) - 8 * C
    ny = e.redMul(d.redISub(nx)).redISub(c8)
    // Z3 = 2 * Y1 * Z1
    nz = this.y.redMul(this.z)
    nz = nz.redIAdd(nz)
  }

  return new ECJPoint(nx, ny, nz)
}

ECJPoint.prototype.dblp = function (pow) {
  if (pow === 0 || this.inf) return this

  var point = this
  for (var i = 0; i < pow; i++) point = point.dbl()

  return point
}

Object.defineProperty(ECJPoint.prototype, 'inf', {
  enumerable: true,
  get: function () {
    return this.z.isZero()
  }
})

module.exports = ECJPoint
