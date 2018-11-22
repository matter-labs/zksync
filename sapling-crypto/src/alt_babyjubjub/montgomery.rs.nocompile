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
    edwards
};

use rand::{
    Rng
};

use std::marker::PhantomData;

// Represents the affine point (X, Y)
pub struct Point<E: JubjubEngine, Subgroup> {
    x: E::Fr,
    y: E::Fr,
    infinity: bool,
    _marker: PhantomData<Subgroup>
}

fn convert_subgroup<E: JubjubEngine, S1, S2>(from: &Point<E, S1>) -> Point<E, S2>
{
    Point {
        x: from.x,
        y: from.y,
        infinity: from.infinity,
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
        match (self.infinity, other.infinity) {
            (true, true) => true,
            (true, false) | (false, true) => false,
            (false, false) => {
                self.x == other.x && self.y == other.y
            }
        }
    }
}

impl<E: JubjubEngine> Point<E, Unknown> {
    pub fn get_for_x(x: E::Fr, sign: bool, params: &E::Params) -> Option<Self>
    {
        // Given an x on the curve, y = sqrt(x^3 + A*x^2 + x)

        let mut x2 = x;
        x2.square();

        let mut rhs = x2;
        rhs.mul_assign(params.montgomery_a());
        rhs.add_assign(&x);
        x2.mul_assign(&x);
        rhs.add_assign(&x2);

        match rhs.sqrt() {
            Some(mut y) => {
                if y.into_repr().is_odd() != sign {
                    y.negate();
                }

                return Some(Point {
                    x: x,
                    y: y,
                    infinity: false,
                    _marker: PhantomData
                })
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
            let x: E::Fr = rng.gen();

            match Self::get_for_x(x, rng.gen(), params) {
                Some(p) => {
                    return p
                },
                None => {}
            }
        }
    }
}

impl<E: JubjubEngine, Subgroup> Point<E, Subgroup> {
    /// Convert from an Edwards point
    pub fn from_edwards(
        e: &edwards::Point<E, Subgroup>,
        params: &E::Params
    ) -> Self
    {
        let (x, y) = e.into_xy();

        if y == E::Fr::one() {
            // The only solution for y = 1 is x = 0. (0, 1) is
            // the neutral element, so we map this to the point
            // at infinity.

            Point::zero()
        } else {
            // The map from a twisted Edwards curve is defined as
            // (x, y) -> (u, v) where
            //      u = (1 + y) / (1 - y)
            //      v = u / x
            //
            // This mapping is not defined for y = 1 and for x = 0.
            //
            // We have that y != 1 above. If x = 0, the only
            // solutions for y are 1 (contradiction) or -1.
            if x.is_zero() {
                // (0, -1) is the point of order two which is not
                // the neutral element, so we map it to (0, 0) which is
                // the only affine point of order 2.

                Point {
                    x: E::Fr::zero(),
                    y: E::Fr::zero(),
                    infinity: false,
                    _marker: PhantomData
                }
            } else {
                // The mapping is defined as above.
                //
                // (x, y) -> (u, v) where
                //      u = (1 + y) / (1 - y)
                //      v = u / x

                let mut u = E::Fr::one();
                u.add_assign(&y);
                {
                    let mut tmp = E::Fr::one();
                    tmp.sub_assign(&y);
                    u.mul_assign(&tmp.inverse().unwrap())
                }

                let mut v = u;
                v.mul_assign(&x.inverse().unwrap());

                // Scale it into the correct curve constants
                v.mul_assign(params.scale());

                Point {
                    x: u,
                    y: v,
                    infinity: false,
                    _marker: PhantomData
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
            y: E::Fr::zero(),
            infinity: true,
            _marker: PhantomData
        }
    }

    pub fn into_xy(&self) -> Option<(E::Fr, E::Fr)>
    {
        if self.infinity {
            None
        } else {
            Some((self.x, self.y))
        }
    }

    #[must_use]
    pub fn negate(&self) -> Self {
        let mut p = self.clone();

        p.y.negate();

        p
    }

    #[must_use]
    pub fn double(&self, params: &E::Params) -> Self {
        if self.infinity {
            return Point::zero();
        }

        // (0, 0) is the point of order 2. Doubling
        // produces the point at infinity.
        if self.y == E::Fr::zero() {
            return Point::zero();
        }

        // This is a standard affine point doubling formula
        // See 4.3.2 The group law for Weierstrass curves
        //     Montgomery curves and the Montgomery Ladder
        //     Daniel J. Bernstein and Tanja Lange

        let mut delta = E::Fr::one();
        {
            let mut tmp = params.montgomery_a().clone();
            tmp.mul_assign(&self.x);
            tmp.double();
            delta.add_assign(&tmp);
        }
        {
            let mut tmp = self.x;
            tmp.square();
            delta.add_assign(&tmp);
            tmp.double();
            delta.add_assign(&tmp);
        }
        {
            let mut tmp = self.y;
            tmp.double();
            delta.mul_assign(&tmp.inverse().expect("y is nonzero so this must be nonzero"));
        }

        let mut x3 = delta;
        x3.square();
        x3.sub_assign(params.montgomery_a());
        x3.sub_assign(&self.x);
        x3.sub_assign(&self.x);

        let mut y3 = x3;
        y3.sub_assign(&self.x);
        y3.mul_assign(&delta);
        y3.add_assign(&self.y);
        y3.negate();

        Point {
            x: x3,
            y: y3,
            infinity: false,
            _marker: PhantomData
        }
    }

    #[must_use]
    pub fn add(&self, other: &Self, params: &E::Params) -> Self
    {
        // This is a standard affine point addition formula
        // See 4.3.2 The group law for Weierstrass curves
        //     Montgomery curves and the Montgomery Ladder
        //     Daniel J. Bernstein and Tanja Lange

        match (self.infinity, other.infinity) {
            (true, true) => Point::zero(),
            (true, false) => other.clone(),
            (false, true) => self.clone(),
            (false, false) => {
                if self.x == other.x {
                    if self.y == other.y {
                        self.double(params)
                    } else {
                        Point::zero()
                    }
                } else {
                    let mut delta = other.y;
                    delta.sub_assign(&self.y);
                    {
                        let mut tmp = other.x;
                        tmp.sub_assign(&self.x);
                        delta.mul_assign(&tmp.inverse().expect("self.x != other.x, so this must be nonzero"));
                    }

                    let mut x3 = delta;
                    x3.square();
                    x3.sub_assign(params.montgomery_a());
                    x3.sub_assign(&self.x);
                    x3.sub_assign(&other.x);

                    let mut y3 = x3;
                    y3.sub_assign(&self.x);
                    y3.mul_assign(&delta);
                    y3.add_assign(&self.y);
                    y3.negate();

                    Point {
                        x: x3,
                        y: y3,
                        infinity: false,
                        _marker: PhantomData
                    }
                }
            }
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
