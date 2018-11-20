use ff::{
    Field,
    SqrtField,
    PrimeField,
    PrimeFieldRepr,
    BitIterator
};

use super::{
    JubjubEngine,
    JubjubParams,
    Unknown,
    PrimeOrder,
    montgomery
};

use rand::{
    Rng
};

use std::marker::PhantomData;

use std::io::{
    self,
    Write,
    Read
};

// Represents the affine point (X/Z, Y/Z) via the extended
// twisted Edwards coordinates.
//
// See "Twisted Edwards Curves Revisited"
//     Huseyin Hisil, Kenneth Koon-Ho Wong, Gary Carter, and Ed Dawson
pub struct Point<E: JubjubEngine, Subgroup> {
    x: E::Fr,
    y: E::Fr,
    t: E::Fr,
    z: E::Fr,
    _marker: PhantomData<Subgroup>
}

fn convert_subgroup<E: JubjubEngine, S1, S2>(from: &Point<E, S1>) -> Point<E, S2>
{
    Point {
        x: from.x,
        y: from.y,
        t: from.t,
        z: from.z,
        _marker: PhantomData
    }
}

impl<E: JubjubEngine> From<Point<E, PrimeOrder>> for Point<E, Unknown>
{
    fn from(p: Point<E, PrimeOrder>) -> Point<E, Unknown>
    {
        convert_subgroup(&p)
    }
}

impl<E: JubjubEngine, Subgroup> Clone for Point<E, Subgroup>
{
    fn clone(&self) -> Self {
        convert_subgroup(self)
    }
}

impl<E: JubjubEngine, Subgroup> PartialEq for Point<E, Subgroup> {
    fn eq(&self, other: &Point<E, Subgroup>) -> bool {
        // p1 = (x1/z1, y1/z1)
        // p2 = (x2/z2, y2/z2)
        // Deciding that these two points are equal is a matter of
        // determining that x1/z1 = x2/z2, or equivalently that
        // x1*z2 = x2*z1, and similarly for y.

        let mut x1 = self.x;
        x1.mul_assign(&other.z);

        let mut y1 = self.y;
        y1.mul_assign(&other.z);

        let mut x2 = other.x;
        x2.mul_assign(&self.z);

        let mut y2 = other.y;
        y2.mul_assign(&self.z);

        x1 == x2 && y1 == y2
    }
}

impl<E: JubjubEngine> Point<E, Unknown> {
    pub fn read<R: Read>(
        reader: R,
        params: &E::Params
    ) -> io::Result<Self>
    {
        let mut y_repr = <E::Fr as PrimeField>::Repr::default();
        y_repr.read_le(reader)?;

        let x_sign = (y_repr.as_ref()[3] >> 63) == 1;
        y_repr.as_mut()[3] &= 0x7fffffffffffffff;

        match E::Fr::from_repr(y_repr) {
            Ok(y) => {
                match Self::get_for_y(y, x_sign, params) {
                    Some(p) => Ok(p),
                    None => {
                        Err(io::Error::new(io::ErrorKind::InvalidInput, "not on curve"))
                    }
                }
            },
            Err(_) => {
                Err(io::Error::new(io::ErrorKind::InvalidInput, "y is not in field"))
            }
        }
    }

    pub fn get_for_y(y: E::Fr, sign: bool, params: &E::Params) -> Option<Self>
    {
        // HERE it' different from jubjub
        // Given a y on the curve, x^2 = (y^2 - 1) / (dy^2 - a)
        // This is defined for all valid y-coordinates,
        // as dy^2 - a = 0 has no solution in Fr.

        // tmp1 = y^2
        let mut tmp1 = y;
        tmp1.square();

        // tmp2 = (y^2 * d) - a
        let mut tmp2 = tmp1;
        tmp2.mul_assign(params.edwards_d());
        tmp2.sub_assign(params.edwards_a()); 
        // tmp2.add_assign(&E::Fr::one());

        // tmp1 = y^2 - 1
        tmp1.sub_assign(&E::Fr::one());

        match tmp2.inverse() {
            Some(tmp2) => {
                // tmp1 = (y^2 - 1) / (dy^2 - a)
                tmp1.mul_assign(&tmp2);

                match tmp1.sqrt() {
                    Some(mut x) => {
                        if x.into_repr().is_odd() != sign {
                            x.negate();
                        }

                        let mut t = x;
                        t.mul_assign(&y);

                        Some(Point {
                            x: x,
                            y: y,
                            t: t,
                            z: E::Fr::one(),
                            _marker: PhantomData
                        })
                    },
                    None => None
                }
            },
            None => None
        }
    }

    /// This guarantees the point is in the prime order subgroup
    #[must_use]
    pub fn mul_by_cofactor(&self, params: &E::Params) -> Point<E, PrimeOrder>
    {
        let tmp = self.double(params)
                      .double(params)
                      .double(params);

        convert_subgroup(&tmp)
    }

    pub fn rand<R: Rng>(rng: &mut R, params: &E::Params) -> Self
    {
        loop {
            let y: E::Fr = rng.gen();

            if let Some(p) = Self::get_for_y(y, rng.gen(), params) {
                return p;
            }
        }
    }
}

impl<E: JubjubEngine, Subgroup> Point<E, Subgroup> {
    pub fn write<W: Write>(
        &self,
        writer: W
    ) -> io::Result<()>
    {
        let (x, y) = self.into_xy();

        assert_eq!(E::Fr::NUM_BITS, 254);

        let x_repr = x.into_repr();
        let mut y_repr = y.into_repr();
        if x_repr.is_odd() {
            y_repr.as_mut()[3] |= 0x8000000000000000u64;
        }

        y_repr.write_le(writer)
    }

    /// Convert from a Montgomery point
    pub fn from_montgomery(
        m: &montgomery::Point<E, Subgroup>,
        params: &E::Params
    ) -> Self
    {
        match m.into_xy() {
            None => {
                // Map the point at infinity to the neutral element.
                Point::zero()
            },
            Some((x, y)) => {
                // The map from a Montgomery curve is defined as:
                // (x, y) -> (u, v) where
                //      u = x / y
                //      v = (x - 1) / (x + 1)
                //
                // This map is not defined for y = 0 and x = -1.
                //
                // y = 0 is a valid point only for x = 0:
                //     y^2 = x^3 + A.x^2 + x
                //       0 = x^3 + A.x^2 + x
                //       0 = x(x^2 + A.x + 1)
                // We have: x = 0  OR  x^2 + A.x + 1 = 0
                //       x^2 + A.x + 1 = 0
                //         (2.x + A)^2 = A^2 - 4 (Complete the square.)
                // The left hand side is a square, and so if A^2 - 4
                // is nonsquare, there is no solution. Indeed, A^2 - 4
                // is nonsquare.
                //
                // (0, 0) is a point of order 2, and so we map it to
                // (0, -1) in the twisted Edwards curve, which is the
                // only point of order 2 that is not the neutral element.
                if y.is_zero() {
                    // This must be the point (0, 0) as above.
                    let mut neg1 = E::Fr::one();
                    neg1.negate();

                    Point {
                        x: E::Fr::zero(),
                        y: neg1,
                        t: E::Fr::zero(),
                        z: E::Fr::one(),
                        _marker: PhantomData
                    }
                } else {
                    // Otherwise, as stated above, the mapping is still
                    // not defined at x = -1. However, x = -1 is not
                    // on the curve when A - 2 is nonsquare:
                    //     y^2 = x^3 + A.x^2 + x
                    //     y^2 = (-1) + A + (-1)
                    //     y^2 = A - 2
                    // Indeed, A - 2 is nonsquare.
                    //
                    // We need to map into (projective) extended twisted
                    // Edwards coordinates (X, Y, T, Z) which represents
                    // the point (X/Z, Y/Z) with Z nonzero and T = XY/Z.
                    //
                    // Thus, we compute...
                    //
                    // u = x(x + 1)
                    // v = y(x - 1)
                    // t = x(x - 1)
                    // z = y(x + 1)  (Cannot be nonzero, as above.)
                    //
                    // ... which represents the point ( x / y , (x - 1) / (x + 1) )
                    // as required by the mapping and preserves the property of
                    // the auxiliary coordinate t.
                    //
                    // We need to scale the coordinate, so u and t will have
                    // an extra factor s.

                    // u = xs
                    let mut u = x;
                    u.mul_assign(params.scale());

                    // v = x - 1
                    let mut v = x;
                    v.sub_assign(&E::Fr::one());

                    // t = xs(x - 1)
                    let mut t = u;
                    t.mul_assign(&v);

                    // z = (x + 1)
                    let mut z = x;
                    z.add_assign(&E::Fr::one());

                    // u = xs(x + 1)
                    u.mul_assign(&z);

                    // z = y(x + 1)
                    z.mul_assign(&y);

                    // v = y(x - 1)
                    v.mul_assign(&y);

                    Point {
                        x: u,
                        y: v,
                        t: t,
                        z: z,
                        _marker: PhantomData
                    }
                }
            }
        }
    }

    /// Attempts to cast this as a prime order element, failing if it's
    /// not in the prime order subgroup.
    pub fn as_prime_order(&self, params: &E::Params) -> Option<Point<E, PrimeOrder>> {
        if self.mul(E::Fs::char(), params) == Point::zero() {
            Some(convert_subgroup(self))
        } else {
            None
        }
    }

    pub fn zero() -> Self {
        Point {
            x: E::Fr::zero(),
            y: E::Fr::one(),
            t: E::Fr::zero(),
            z: E::Fr::one(),
            _marker: PhantomData
        }
    }

    pub fn into_xy(&self) -> (E::Fr, E::Fr)
    {
        let zinv = self.z.inverse().unwrap();

        let mut x = self.x;
        x.mul_assign(&zinv);

        let mut y = self.y;
        y.mul_assign(&zinv);

        (x, y)
    }

    #[must_use]
    pub fn negate(&self) -> Self {
        let mut p = self.clone();

        p.x.negate();
        p.t.negate();

        p
    }

    #[must_use]
    pub fn double(&self, params: &E::Params) -> Self {
        // See "Twisted Edwards Curves Revisited"
        //     Huseyin Hisil, Kenneth Koon-Ho Wong, Gary Carter, and Ed Dawson
        //     Section 3.3
        //     http://hyperelliptic.org/EFD/g1p/auto-twisted-extended.html#doubling-dbl-2008-hwcd

        // A = X1^2
        let mut a = self.x;
        a.square();

        // B = Y1^2
        let mut b = self.y;
        b.square();

        // C = 2*Z1^2
        let mut c = self.z;
        c.square();
        c.double();

        // HERE it's different from jubjub
        // D = a*A
        let mut d = a;
        d.mul_assign(params.edwards_a());
        // d.negate();

        // E = (X1+Y1)^2 - A - B
        let mut e = self.x;
        e.add_assign(&self.y);
        e.square();
        // HERE it's different from jubjub
        e.sub_assign(&a);
        // e.add_assign(&d); // -A = D
        e.sub_assign(&b);

        // G = D+B
        let mut g = d;
        g.add_assign(&b);

        // F = G-C
        let mut f = g;
        f.sub_assign(&c);

        // H = D-B
        let mut h = d;
        h.sub_assign(&b);

        // X3 = E*F
        let mut x3 = e;
        x3.mul_assign(&f);

        // Y3 = G*H
        let mut y3 = g;
        y3.mul_assign(&h);

        // T3 = E*H
        let mut t3 = e;
        t3.mul_assign(&h);

        // Z3 = F*G
        let mut z3 = f;
        z3.mul_assign(&g);

        Point {
            x: x3,
            y: y3,
            t: t3,
            z: z3,
            _marker: PhantomData
        }
    }

    #[must_use]
    pub fn add(&self, other: &Self, params: &E::Params) -> Self
    {
        // See "Twisted Edwards Curves Revisited"
        //     Huseyin Hisil, Kenneth Koon-Ho Wong, Gary Carter, and Ed Dawson
        //     3.1 Unified Addition in E^e

        // A = x1 * x2
        let mut a = self.x;
        a.mul_assign(&other.x);

        // B = y1 * y2
        let mut b = self.y;
        b.mul_assign(&other.y);

        // C = d * t1 * t2
        let mut c = params.edwards_d().clone();
        c.mul_assign(&self.t);
        c.mul_assign(&other.t);

        // D = z1 * z2
        let mut d = self.z;
        d.mul_assign(&other.z);

        // HERE it's different from jubjub
        // H = B - aA
        let mut a_a = a;
        a_a.mul_assign(params.edwards_a());

        let mut h = b;
        h.sub_assign(&a_a);

        // E = (x1 + y1) * (x2 + y2) - A - B
        //   = (x1 + y1) * (x2 + y2) - H
        let mut e = self.x;
        e.add_assign(&self.y);
        {
            let mut tmp = other.x;
            tmp.add_assign(&other.y);
            e.mul_assign(&tmp);
        }
        e.sub_assign(&h);

        // F = D - C
        let mut f = d;
        f.sub_assign(&c);

        // G = D + C
        let mut g = d;
        g.add_assign(&c);

        // x3 = E * F
        let mut x3 = e;
        x3.mul_assign(&f);

        // y3 = G * H
        let mut y3 = g;
        y3.mul_assign(&h);

        // t3 = E * H
        let mut t3 = e;
        t3.mul_assign(&h);

        // z3 = F * G
        let mut z3 = f;
        z3.mul_assign(&g);

        Point {
            x: x3,
            y: y3,
            t: t3,
            z: z3,
            _marker: PhantomData
        }
    }

    #[must_use]
    pub fn mul<S: Into<<E::Fs as PrimeField>::Repr>>(
        &self,
        scalar: S,
        params: &E::Params
    ) -> Self
    {
        // Standard double-and-add scalar multiplication

        let mut res = Self::zero();

        for b in BitIterator::new(scalar.into()) {
            res = res.double(params);

            if b {
                res = res.add(self, params);
            }
        }

        res
    }
}
