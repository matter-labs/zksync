macro_rules! curve_impl {
    (
        $name:expr,
        $projective:ident,
        $affine:ident,
        $prepared:ident,
        $basefield:ident,
        $scalarfield:ident,
        $uncompressed:ident,
        $compressed:ident,
        $pairing:ident
    ) => {
        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        pub struct $affine {
            pub(crate) x: $basefield,
            pub(crate) y: $basefield,
            pub(crate) infinity: bool
        }

        impl ::std::fmt::Display for $affine
        {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                if self.infinity {
                    write!(f, "{}(Infinity)", $name)
                } else {
                    write!(f, "{}(x={}, y={})", $name, self.x, self.y)
                }
            }
        }

        #[derive(Copy, Clone, Debug, Eq)]
        pub struct $projective {
           pub(crate) x: $basefield,
           pub(crate) y: $basefield,
           pub(crate) z: $basefield
        }

        impl ::std::fmt::Display for $projective
        {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{}", self.into_affine())
            }
        }

        impl PartialEq for $projective {
            fn eq(&self, other: &$projective) -> bool {
                if self.is_zero() {
                    return other.is_zero();
                }

                if other.is_zero() {
                    return false;
                }

                // The points (X, Y, Z) and (X', Y', Z')
                // are equal when (X * Z^2) = (X' * Z'^2)
                // and (Y * Z^3) = (Y' * Z'^3).

                let mut z1 = self.z;
                z1.square();
                let mut z2 = other.z;
                z2.square();

                let mut tmp1 = self.x;
                tmp1.mul_assign(&z2);

                let mut tmp2 = other.x;
                tmp2.mul_assign(&z1);

                if tmp1 != tmp2 {
                    return false;
                }

                z1.mul_assign(&self.z);
                z2.mul_assign(&other.z);
                z2.mul_assign(&self.y);
                z1.mul_assign(&other.y);

                if z1 != z2 {
                    return false;
                }

                true
            }
        }

        impl $affine {
            fn mul_bits<S: AsRef<[u64]>>(&self, bits: BitIterator<S>) -> $projective {
                let mut res = $projective::zero();
                for i in bits {
                    res.double();
                    if i { res.add_assign_mixed(self) }
                }
                res
            }

            /// Attempts to construct an affine point given an x-coordinate. The
            /// point is not guaranteed to be in the prime order subgroup.
            ///
            /// If and only if `greatest` is set will the lexicographically
            /// largest y-coordinate be selected.
            fn get_point_from_x(x: $basefield, greatest: bool) -> Option<$affine> {
                // Compute x^3 + b
                let mut x3b = x;
                x3b.square();
                x3b.mul_assign(&x);
                x3b.add_assign(&$affine::get_coeff_b());

                x3b.sqrt().map(|y| {
                    let mut negy = y;
                    negy.negate();

                    $affine {
                        x: x,
                        y: if (y < negy) ^ greatest {
                            y
                        } else {
                            negy
                        },
                        infinity: false
                    }
                })
            }

            fn is_on_curve(&self) -> bool {
                if self.is_zero() {
                    true
                } else {
                    // Check that the point is on the curve
                    let mut y2 = self.y;
                    y2.square();

                    let mut x3b = self.x;
                    x3b.square();
                    x3b.mul_assign(&self.x);
                    x3b.add_assign(&Self::get_coeff_b());

                    y2 == x3b
                }
            }

            fn is_in_correct_subgroup_assuming_on_curve(&self) -> bool {
                self.mul($scalarfield::char()).is_zero()
            }
        }

        impl CurveAffine for $affine {
            type Engine = Bls12;
            type Scalar = $scalarfield;
            type Base = $basefield;
            type Prepared = $prepared;
            type Projective = $projective;
            type Uncompressed = $uncompressed;
            type Compressed = $compressed;
            type Pair = $pairing;
            type PairingResult = Fq12;

            fn zero() -> Self {
                $affine {
                    x: $basefield::zero(),
                    y: $basefield::one(),
                    infinity: true
                }
            }

            fn one() -> Self {
                Self::get_generator()
            }

            fn is_zero(&self) -> bool {
                self.infinity
            }

            fn mul<S: Into<<Self::Scalar as PrimeField>::Repr>>(&self, by: S) -> $projective {
                let bits = BitIterator::new(by.into());
                self.mul_bits(bits)
            }

            fn negate(&mut self) {
                if !self.is_zero() {
                    self.y.negate();
                }
            }

            fn prepare(&self) -> Self::Prepared {
                $prepared::from_affine(*self)
            }

            fn pairing_with(&self, other: &Self::Pair) -> Self::PairingResult {
                self.perform_pairing(other)
            }

            fn into_projective(&self) -> $projective {
                (*self).into()
            }

        }

        impl Rand for $projective {
            fn rand<R: Rng>(rng: &mut R) -> Self {
                loop {
                    let x = rng.gen();
                    let greatest = rng.gen();

                    if let Some(p) = $affine::get_point_from_x(x, greatest) {
                        let p = p.scale_by_cofactor();

                        if !p.is_zero() {
                            return p;
                        }
                    }
                }
            }
        }

        impl CurveProjective for $projective {
            type Engine = Bls12;
            type Scalar = $scalarfield;
            type Base = $basefield;
            type Affine = $affine;

            // The point at infinity is always represented by
            // Z = 0.
            fn zero() -> Self {
                $projective {
                    x: $basefield::zero(),
                    y: $basefield::one(),
                    z: $basefield::zero()
                }
            }

            fn one() -> Self {
                $affine::one().into()
            }

            // The point at infinity is always represented by
            // Z = 0.
            fn is_zero(&self) -> bool {
                self.z.is_zero()
            }

            fn is_normalized(&self) -> bool {
                self.is_zero() || self.z == $basefield::one()
            }

            fn batch_normalization(v: &mut [Self])
            {
                // Montgomery’s Trick and Fast Implementation of Masked AES
                // Genelle, Prouff and Quisquater
                // Section 3.2

                // First pass: compute [a, ab, abc, ...]
                let mut prod = Vec::with_capacity(v.len());
                let mut tmp = $basefield::one();
                for g in v.iter_mut()
                          // Ignore normalized elements
                          .filter(|g| !g.is_normalized())
                {
                    tmp.mul_assign(&g.z);
                    prod.push(tmp);
                }

                // Invert `tmp`.
                tmp = tmp.inverse().unwrap(); // Guaranteed to be nonzero.

                // Second pass: iterate backwards to compute inverses
                for (g, s) in v.iter_mut()
                               // Backwards
                               .rev()
                               // Ignore normalized elements
                               .filter(|g| !g.is_normalized())
                               // Backwards, skip last element, fill in one for last term.
                               .zip(prod.into_iter().rev().skip(1).chain(Some($basefield::one())))
                {
                    // tmp := tmp * g.z; g.z := tmp * s = 1/z
                    let mut newtmp = tmp;
                    newtmp.mul_assign(&g.z);
                    g.z = tmp;
                    g.z.mul_assign(&s);
                    tmp = newtmp;
                }

                // Perform affine transformations
                for g in v.iter_mut()
                          .filter(|g| !g.is_normalized())
                {
                    let mut z = g.z; // 1/z
                    z.square(); // 1/z^2
                    g.x.mul_assign(&z); // x/z^2
                    z.mul_assign(&g.z); // 1/z^3
                    g.y.mul_assign(&z); // y/z^3
                    g.z = $basefield::one(); // z = 1
                }
            }

            fn double(&mut self) {
                if self.is_zero() {
                    return;
                }

                // Other than the point at infinity, no points on E or E'
                // can double to equal the point at infinity, as y=0 is
                // never true for points on the curve. (-4 and -4u-4
                // are not cubic residue in their respective fields.)

                // http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#doubling-dbl-2009-l

                // A = X1^2
                let mut a = self.x;
                a.square();

                // B = Y1^2
                let mut b = self.y;
                b.square();

                // C = B^2
                let mut c = b;
                c.square();

                // D = 2*((X1+B)2-A-C)
                let mut d = self.x;
                d.add_assign(&b);
                d.square();
                d.sub_assign(&a);
                d.sub_assign(&c);
                d.double();

                // E = 3*A
                let mut e = a;
                e.double();
                e.add_assign(&a);

                // F = E^2
                let mut f = e;
                f.square();

                // Z3 = 2*Y1*Z1
                self.z.mul_assign(&self.y);
                self.z.double();

                // X3 = F-2*D
                self.x = f;
                self.x.sub_assign(&d);
                self.x.sub_assign(&d);

                // Y3 = E*(D-X3)-8*C
                self.y = d;
                self.y.sub_assign(&self.x);
                self.y.mul_assign(&e);
                c.double();
                c.double();
                c.double();
                self.y.sub_assign(&c);
            }

            fn add_assign(&mut self, other: &Self) {
                if self.is_zero() {
                    *self = *other;
                    return;
                }

                if other.is_zero() {
                    return;
                }

                // http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#addition-add-2007-bl

                // Z1Z1 = Z1^2
                let mut z1z1 = self.z;
                z1z1.square();

                // Z2Z2 = Z2^2
                let mut z2z2 = other.z;
                z2z2.square();

                // U1 = X1*Z2Z2
                let mut u1 = self.x;
                u1.mul_assign(&z2z2);

                // U2 = X2*Z1Z1
                let mut u2 = other.x;
                u2.mul_assign(&z1z1);

                // S1 = Y1*Z2*Z2Z2
                let mut s1 = self.y;
                s1.mul_assign(&other.z);
                s1.mul_assign(&z2z2);

                // S2 = Y2*Z1*Z1Z1
                let mut s2 = other.y;
                s2.mul_assign(&self.z);
                s2.mul_assign(&z1z1);

                if u1 == u2 && s1 == s2 {
                    // The two points are equal, so we double.
                    self.double();
                } else {
                    // If we're adding -a and a together, self.z becomes zero as H becomes zero.

                    // H = U2-U1
                    let mut h = u2;
                    h.sub_assign(&u1);

                    // I = (2*H)^2
                    let mut i = h;
                    i.double();
                    i.square();

                    // J = H*I
                    let mut j = h;
                    j.mul_assign(&i);

                    // r = 2*(S2-S1)
                    let mut r = s2;
                    r.sub_assign(&s1);
                    r.double();

                    // V = U1*I
                    let mut v = u1;
                    v.mul_assign(&i);

                    // X3 = r^2 - J - 2*V
                    self.x = r;
                    self.x.square();
                    self.x.sub_assign(&j);
                    self.x.sub_assign(&v);
                    self.x.sub_assign(&v);

                    // Y3 = r*(V - X3) - 2*S1*J
                    self.y = v;
                    self.y.sub_assign(&self.x);
                    self.y.mul_assign(&r);
                    s1.mul_assign(&j); // S1 = S1 * J * 2
                    s1.double();
                    self.y.sub_assign(&s1);

                    // Z3 = ((Z1+Z2)^2 - Z1Z1 - Z2Z2)*H
                    self.z.add_assign(&other.z);
                    self.z.square();
                    self.z.sub_assign(&z1z1);
                    self.z.sub_assign(&z2z2);
                    self.z.mul_assign(&h);
                }
            }

            fn add_assign_mixed(&mut self, other: &Self::Affine) {
                if other.is_zero() {
                    return;
                }

                if self.is_zero() {
                    self.x = other.x;
                    self.y = other.y;
                    self.z = $basefield::one();
                    return;
                }

                // http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#addition-madd-2007-bl

                // Z1Z1 = Z1^2
                let mut z1z1 = self.z;
                z1z1.square();

                // U2 = X2*Z1Z1
                let mut u2 = other.x;
                u2.mul_assign(&z1z1);

                // S2 = Y2*Z1*Z1Z1
                let mut s2 = other.y;
                s2.mul_assign(&self.z);
                s2.mul_assign(&z1z1);

                if self.x == u2 && self.y == s2 {
                    // The two points are equal, so we double.
                    self.double();
                } else {
                    // If we're adding -a and a together, self.z becomes zero as H becomes zero.

                    // H = U2-X1
                    let mut h = u2;
                    h.sub_assign(&self.x);

                    // HH = H^2
                    let mut hh = h;
                    hh.square();

                    // I = 4*HH
                    let mut i = hh;
                    i.double();
                    i.double();

                    // J = H*I
                    let mut j = h;
                    j.mul_assign(&i);

                    // r = 2*(S2-Y1)
                    let mut r = s2;
                    r.sub_assign(&self.y);
                    r.double();

                    // V = X1*I
                    let mut v = self.x;
                    v.mul_assign(&i);

                    // X3 = r^2 - J - 2*V
                    self.x = r;
                    self.x.square();
                    self.x.sub_assign(&j);
                    self.x.sub_assign(&v);
                    self.x.sub_assign(&v);

                    // Y3 = r*(V-X3)-2*Y1*J
                    j.mul_assign(&self.y); // J = 2*Y1*J
                    j.double();
                    self.y = v;
                    self.y.sub_assign(&self.x);
                    self.y.mul_assign(&r);
                    self.y.sub_assign(&j);

                    // Z3 = (Z1+H)^2-Z1Z1-HH
                    self.z.add_assign(&h);
                    self.z.square();
                    self.z.sub_assign(&z1z1);
                    self.z.sub_assign(&hh);
                }
            }

            fn negate(&mut self) {
                if !self.is_zero() {
                    self.y.negate()
                }
            }

            fn mul_assign<S: Into<<Self::Scalar as PrimeField>::Repr>>(&mut self, other: S) {
                let mut res = Self::zero();

                let mut found_one = false;

                for i in BitIterator::new(other.into())
                {
                    if found_one {
                        res.double();
                    } else {
                        found_one = i;
                    }

                    if i {
                        res.add_assign(self);
                    }
                }

                *self = res;
            }

            fn into_affine(&self) -> $affine {
                (*self).into()
            }

            fn recommended_wnaf_for_scalar(scalar: <Self::Scalar as PrimeField>::Repr) -> usize {
                Self::empirical_recommended_wnaf_for_scalar(scalar)
            }

            fn recommended_wnaf_for_num_scalars(num_scalars: usize) -> usize {
                Self::empirical_recommended_wnaf_for_num_scalars(num_scalars)
            }
        }

        // The affine point X, Y is represented in the jacobian
        // coordinates with Z = 1.
        impl From<$affine> for $projective {
            fn from(p: $affine) -> $projective {
                if p.is_zero() {
                    $projective::zero()
                } else {
                    $projective {
                        x: p.x,
                        y: p.y,
                        z: $basefield::one()
                    }
                }
            }
        }

        // The projective point X, Y, Z is represented in the affine
        // coordinates as X/Z^2, Y/Z^3.
        impl From<$projective> for $affine {
            fn from(p: $projective) -> $affine {
                if p.is_zero() {
                    $affine::zero()
                } else if p.z == $basefield::one() {
                    // If Z is one, the point is already normalized.
                    $affine {
                        x: p.x,
                        y: p.y,
                        infinity: false
                    }
                } else {
                    // Z is nonzero, so it must have an inverse in a field.
                    let zinv = p.z.inverse().unwrap();
                    let mut zinv_powered = zinv;
                    zinv_powered.square();

                    // X/Z^2
                    let mut x = p.x;
                    x.mul_assign(&zinv_powered);

                    // Y/Z^3
                    let mut y = p.y;
                    zinv_powered.mul_assign(&zinv);
                    y.mul_assign(&zinv_powered);

                    $affine {
                        x: x,
                        y: y,
                        infinity: false
                    }
                }
            }
        }
    }
}

pub mod g1 {
    use super::super::{Bls12, Fq, Fq12, FqRepr, Fr, FrRepr};
    use super::g2::G2Affine;
    use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr, SqrtField};
    use rand::{Rand, Rng};
    use std::fmt;
    use {CurveAffine, CurveProjective, EncodedPoint, Engine, GroupDecodingError};

    curve_impl!(
        "G1",
        G1,
        G1Affine,
        G1Prepared,
        Fq,
        Fr,
        G1Uncompressed,
        G1Compressed,
        G2Affine
    );

    #[derive(Copy, Clone)]
    pub struct G1Uncompressed([u8; 96]);

    impl AsRef<[u8]> for G1Uncompressed {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    impl AsMut<[u8]> for G1Uncompressed {
        fn as_mut(&mut self) -> &mut [u8] {
            &mut self.0
        }
    }

    impl fmt::Debug for G1Uncompressed {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            self.0[..].fmt(formatter)
        }
    }

    impl EncodedPoint for G1Uncompressed {
        type Affine = G1Affine;

        fn empty() -> Self {
            G1Uncompressed([0; 96])
        }
        fn size() -> usize {
            96
        }
        fn into_affine(&self) -> Result<G1Affine, GroupDecodingError> {
            let affine = self.into_affine_unchecked()?;

            if !affine.is_on_curve() {
                Err(GroupDecodingError::NotOnCurve)
            } else if !affine.is_in_correct_subgroup_assuming_on_curve() {
                Err(GroupDecodingError::NotInSubgroup)
            } else {
                Ok(affine)
            }
        }
        fn into_affine_unchecked(&self) -> Result<G1Affine, GroupDecodingError> {
            // Create a copy of this representation.
            let mut copy = self.0;

            if copy[0] & (1 << 7) != 0 {
                // Distinguisher bit is set, but this should be uncompressed!
                return Err(GroupDecodingError::UnexpectedCompressionMode);
            }

            if copy[0] & (1 << 6) != 0 {
                // This is the point at infinity, which means that if we mask away
                // the first two bits, the entire representation should consist
                // of zeroes.
                copy[0] &= 0x3f;

                if copy.iter().all(|b| *b == 0) {
                    Ok(G1Affine::zero())
                } else {
                    Err(GroupDecodingError::UnexpectedInformation)
                }
            } else {
                if copy[0] & (1 << 5) != 0 {
                    // The bit indicating the y-coordinate should be lexicographically
                    // largest is set, but this is an uncompressed element.
                    return Err(GroupDecodingError::UnexpectedInformation);
                }

                // Unset the three most significant bits.
                copy[0] &= 0x1f;

                let mut x = FqRepr([0; 6]);
                let mut y = FqRepr([0; 6]);

                {
                    let mut reader = &copy[..];

                    x.read_be(&mut reader).unwrap();
                    y.read_be(&mut reader).unwrap();
                }

                Ok(G1Affine {
                    x: Fq::from_repr(x).map_err(|e| {
                        GroupDecodingError::CoordinateDecodingError("x coordinate", e)
                    })?,
                    y: Fq::from_repr(y).map_err(|e| {
                        GroupDecodingError::CoordinateDecodingError("y coordinate", e)
                    })?,
                    infinity: false,
                })
            }
        }
        fn from_affine(affine: G1Affine) -> Self {
            let mut res = Self::empty();

            if affine.is_zero() {
                // Set the second-most significant bit to indicate this point
                // is at infinity.
                res.0[0] |= 1 << 6;
            } else {
                let mut writer = &mut res.0[..];

                affine.x.into_repr().write_be(&mut writer).unwrap();
                affine.y.into_repr().write_be(&mut writer).unwrap();
            }

            res
        }
    }

    #[derive(Copy, Clone)]
    pub struct G1Compressed([u8; 48]);

    impl AsRef<[u8]> for G1Compressed {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    impl AsMut<[u8]> for G1Compressed {
        fn as_mut(&mut self) -> &mut [u8] {
            &mut self.0
        }
    }

    impl fmt::Debug for G1Compressed {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            self.0[..].fmt(formatter)
        }
    }

    impl EncodedPoint for G1Compressed {
        type Affine = G1Affine;

        fn empty() -> Self {
            G1Compressed([0; 48])
        }
        fn size() -> usize {
            48
        }
        fn into_affine(&self) -> Result<G1Affine, GroupDecodingError> {
            let affine = self.into_affine_unchecked()?;

            // NB: Decompression guarantees that it is on the curve already.

            if !affine.is_in_correct_subgroup_assuming_on_curve() {
                Err(GroupDecodingError::NotInSubgroup)
            } else {
                Ok(affine)
            }
        }
        fn into_affine_unchecked(&self) -> Result<G1Affine, GroupDecodingError> {
            // Create a copy of this representation.
            let mut copy = self.0;

            if copy[0] & (1 << 7) == 0 {
                // Distinguisher bit isn't set.
                return Err(GroupDecodingError::UnexpectedCompressionMode);
            }

            if copy[0] & (1 << 6) != 0 {
                // This is the point at infinity, which means that if we mask away
                // the first two bits, the entire representation should consist
                // of zeroes.
                copy[0] &= 0x3f;

                if copy.iter().all(|b| *b == 0) {
                    Ok(G1Affine::zero())
                } else {
                    Err(GroupDecodingError::UnexpectedInformation)
                }
            } else {
                // Determine if the intended y coordinate must be greater
                // lexicographically.
                let greatest = copy[0] & (1 << 5) != 0;

                // Unset the three most significant bits.
                copy[0] &= 0x1f;

                let mut x = FqRepr([0; 6]);

                {
                    let mut reader = &copy[..];

                    x.read_be(&mut reader).unwrap();
                }

                // Interpret as Fq element.
                let x = Fq::from_repr(x)
                    .map_err(|e| GroupDecodingError::CoordinateDecodingError("x coordinate", e))?;

                G1Affine::get_point_from_x(x, greatest).ok_or(GroupDecodingError::NotOnCurve)
            }
        }
        fn from_affine(affine: G1Affine) -> Self {
            let mut res = Self::empty();

            if affine.is_zero() {
                // Set the second-most significant bit to indicate this point
                // is at infinity.
                res.0[0] |= 1 << 6;
            } else {
                {
                    let mut writer = &mut res.0[..];

                    affine.x.into_repr().write_be(&mut writer).unwrap();
                }

                let mut negy = affine.y;
                negy.negate();

                // Set the third most significant bit if the correct y-coordinate
                // is lexicographically largest.
                if affine.y > negy {
                    res.0[0] |= 1 << 5;
                }
            }

            // Set highest bit to distinguish this as a compressed element.
            res.0[0] |= 1 << 7;

            res
        }
    }

    impl G1Affine {
        fn scale_by_cofactor(&self) -> G1 {
            // G1 cofactor = (x - 1)^2 / 3  = 76329603384216526031706109802092473003
            let cofactor = BitIterator::new([0x8c00aaab0000aaab, 0x396c8c005555e156]);
            self.mul_bits(cofactor)
        }

        fn get_generator() -> Self {
            G1Affine {
                x: super::super::fq::G1_GENERATOR_X,
                y: super::super::fq::G1_GENERATOR_Y,
                infinity: false,
            }
        }

        fn get_coeff_b() -> Fq {
            super::super::fq::B_COEFF
        }

        fn perform_pairing(&self, other: &G2Affine) -> Fq12 {
            super::super::Bls12::pairing(*self, *other)
        }
    }

    impl G1 {
        fn empirical_recommended_wnaf_for_scalar(scalar: FrRepr) -> usize {
            let num_bits = scalar.num_bits() as usize;

            if num_bits >= 130 {
                4
            } else if num_bits >= 34 {
                3
            } else {
                2
            }
        }

        fn empirical_recommended_wnaf_for_num_scalars(num_scalars: usize) -> usize {
            const RECOMMENDATIONS: [usize; 12] =
                [1, 3, 7, 20, 43, 120, 273, 563, 1630, 3128, 7933, 62569];

            let mut ret = 4;
            for r in &RECOMMENDATIONS {
                if num_scalars > *r {
                    ret += 1;
                } else {
                    break;
                }
            }

            ret
        }
    }

    #[derive(Clone, Debug)]
    pub struct G1Prepared(pub(crate) G1Affine);

    impl G1Prepared {
        pub fn is_zero(&self) -> bool {
            self.0.is_zero()
        }

        pub fn from_affine(p: G1Affine) -> Self {
            G1Prepared(p)
        }
    }

    #[test]
    fn g1_generator() {
        use SqrtField;

        let mut x = Fq::zero();
        let mut i = 0;
        loop {
            // y^2 = x^3 + b
            let mut rhs = x;
            rhs.square();
            rhs.mul_assign(&x);
            rhs.add_assign(&G1Affine::get_coeff_b());

            if let Some(y) = rhs.sqrt() {
                let yrepr = y.into_repr();
                let mut negy = y;
                negy.negate();
                let negyrepr = negy.into_repr();

                let p = G1Affine {
                    x: x,
                    y: if yrepr < negyrepr { y } else { negy },
                    infinity: false,
                };
                assert!(!p.is_in_correct_subgroup_assuming_on_curve());

                let g1 = p.scale_by_cofactor();
                if !g1.is_zero() {
                    assert_eq!(i, 4);
                    let g1 = G1Affine::from(g1);

                    assert!(g1.is_in_correct_subgroup_assuming_on_curve());

                    assert_eq!(g1, G1Affine::one());
                    break;
                }
            }

            i += 1;
            x.add_assign(&Fq::one());
        }
    }

    #[test]
    fn g1_test_is_valid() {
        // Reject point on isomorphic twist (b = 24)
        {
            let p = G1Affine {
                x: Fq::from_repr(FqRepr([
                    0xc58d887b66c035dc,
                    0x10cbfd301d553822,
                    0xaf23e064f1131ee5,
                    0x9fe83b1b4a5d648d,
                    0xf583cc5a508f6a40,
                    0xc3ad2aefde0bb13,
                ])).unwrap(),
                y: Fq::from_repr(FqRepr([
                    0x60aa6f9552f03aae,
                    0xecd01d5181300d35,
                    0x8af1cdb8aa8ce167,
                    0xe760f57922998c9d,
                    0x953703f5795a39e5,
                    0xfe3ae0922df702c,
                ])).unwrap(),
                infinity: false,
            };
            assert!(!p.is_on_curve());
            assert!(p.is_in_correct_subgroup_assuming_on_curve());
        }

        // Reject point on a twist (b = 3)
        {
            let p = G1Affine {
                x: Fq::from_repr(FqRepr([
                    0xee6adf83511e15f5,
                    0x92ddd328f27a4ba6,
                    0xe305bd1ac65adba7,
                    0xea034ee2928b30a8,
                    0xbd8833dc7c79a7f7,
                    0xe45c9f0c0438675,
                ])).unwrap(),
                y: Fq::from_repr(FqRepr([
                    0x3b450eb1ab7b5dad,
                    0xa65cb81e975e8675,
                    0xaa548682b21726e5,
                    0x753ddf21a2601d20,
                    0x532d0b640bd3ff8b,
                    0x118d2c543f031102,
                ])).unwrap(),
                infinity: false,
            };
            assert!(!p.is_on_curve());
            assert!(!p.is_in_correct_subgroup_assuming_on_curve());
        }

        // Reject point in an invalid subgroup
        // There is only one r-order subgroup, as r does not divide the cofactor.
        {
            let p = G1Affine {
                x: Fq::from_repr(FqRepr([
                    0x76e1c971c6db8fe8,
                    0xe37e1a610eff2f79,
                    0x88ae9c499f46f0c0,
                    0xf35de9ce0d6b4e84,
                    0x265bddd23d1dec54,
                    0x12a8778088458308,
                ])).unwrap(),
                y: Fq::from_repr(FqRepr([
                    0x8a22defa0d526256,
                    0xc57ca55456fcb9ae,
                    0x1ba194e89bab2610,
                    0x921beef89d4f29df,
                    0x5b6fda44ad85fa78,
                    0xed74ab9f302cbe0,
                ])).unwrap(),
                infinity: false,
            };
            assert!(p.is_on_curve());
            assert!(!p.is_in_correct_subgroup_assuming_on_curve());
        }
    }

    #[test]
    fn test_g1_addition_correctness() {
        let mut p = G1 {
            x: Fq::from_repr(FqRepr([
                0x47fd1f891d6e8bbf,
                0x79a3b0448f31a2aa,
                0x81f3339e5f9968f,
                0x485e77d50a5df10d,
                0x4c6fcac4b55fd479,
                0x86ed4d9906fb064,
            ])).unwrap(),
            y: Fq::from_repr(FqRepr([
                0xd25ee6461538c65,
                0x9f3bbb2ecd3719b9,
                0xa06fd3f1e540910d,
                0xcefca68333c35288,
                0x570c8005f8573fa6,
                0x152ca696fe034442,
            ])).unwrap(),
            z: Fq::one(),
        };

        p.add_assign(&G1 {
            x: Fq::from_repr(FqRepr([
                0xeec78f3096213cbf,
                0xa12beb1fea1056e6,
                0xc286c0211c40dd54,
                0x5f44314ec5e3fb03,
                0x24e8538737c6e675,
                0x8abd623a594fba8,
            ])).unwrap(),
            y: Fq::from_repr(FqRepr([
                0x6b0528f088bb7044,
                0x2fdeb5c82917ff9e,
                0x9a5181f2fac226ad,
                0xd65104c6f95a872a,
                0x1f2998a5a9c61253,
                0xe74846154a9e44,
            ])).unwrap(),
            z: Fq::one(),
        });

        let p = G1Affine::from(p);

        assert_eq!(
            p,
            G1Affine {
                x: Fq::from_repr(FqRepr([
                    0x6dd3098f22235df,
                    0xe865d221c8090260,
                    0xeb96bb99fa50779f,
                    0xc4f9a52a428e23bb,
                    0xd178b28dd4f407ef,
                    0x17fb8905e9183c69
                ])).unwrap(),
                y: Fq::from_repr(FqRepr([
                    0xd0de9d65292b7710,
                    0xf6a05f2bcf1d9ca7,
                    0x1040e27012f20b64,
                    0xeec8d1a5b7466c58,
                    0x4bc362649dce6376,
                    0x430cbdc5455b00a
                ])).unwrap(),
                infinity: false,
            }
        );
    }

    #[test]
    fn test_g1_doubling_correctness() {
        let mut p = G1 {
            x: Fq::from_repr(FqRepr([
                0x47fd1f891d6e8bbf,
                0x79a3b0448f31a2aa,
                0x81f3339e5f9968f,
                0x485e77d50a5df10d,
                0x4c6fcac4b55fd479,
                0x86ed4d9906fb064,
            ])).unwrap(),
            y: Fq::from_repr(FqRepr([
                0xd25ee6461538c65,
                0x9f3bbb2ecd3719b9,
                0xa06fd3f1e540910d,
                0xcefca68333c35288,
                0x570c8005f8573fa6,
                0x152ca696fe034442,
            ])).unwrap(),
            z: Fq::one(),
        };

        p.double();

        let p = G1Affine::from(p);

        assert_eq!(
            p,
            G1Affine {
                x: Fq::from_repr(FqRepr([
                    0xf939ddfe0ead7018,
                    0x3b03942e732aecb,
                    0xce0e9c38fdb11851,
                    0x4b914c16687dcde0,
                    0x66c8baf177d20533,
                    0xaf960cff3d83833
                ])).unwrap(),
                y: Fq::from_repr(FqRepr([
                    0x3f0675695f5177a8,
                    0x2b6d82ae178a1ba0,
                    0x9096380dd8e51b11,
                    0x1771a65b60572f4e,
                    0x8b547c1313b27555,
                    0x135075589a687b1e
                ])).unwrap(),
                infinity: false,
            }
        );
    }

    #[test]
    fn test_g1_same_y() {
        // Test the addition of two points with different x coordinates
        // but the same y coordinate.

        // x1 = 128100205326445210408953809171070606737678357140298133325128175840781723996595026100005714405541449960643523234125
        // x2 = 3821408151224848222394078037104966877485040835569514006839342061575586899845797797516352881516922679872117658572470
        // y = 2291134451313223670499022936083127939567618746216464377735567679979105510603740918204953301371880765657042046687078

        let a = G1Affine {
            x: Fq::from_repr(FqRepr([
                0xea431f2cc38fc94d,
                0x3ad2354a07f5472b,
                0xfe669f133f16c26a,
                0x71ffa8021531705,
                0x7418d484386d267,
                0xd5108d8ff1fbd6,
            ])).unwrap(),
            y: Fq::from_repr(FqRepr([
                0xa776ccbfe9981766,
                0x255632964ff40f4a,
                0xc09744e650b00499,
                0x520f74773e74c8c3,
                0x484c8fc982008f0,
                0xee2c3d922008cc6,
            ])).unwrap(),
            infinity: false,
        };

        let b = G1Affine {
            x: Fq::from_repr(FqRepr([
                0xe06cdb156b6356b6,
                0xd9040b2d75448ad9,
                0xe702f14bb0e2aca5,
                0xc6e05201e5f83991,
                0xf7c75910816f207c,
                0x18d4043e78103106,
            ])).unwrap(),
            y: Fq::from_repr(FqRepr([
                0xa776ccbfe9981766,
                0x255632964ff40f4a,
                0xc09744e650b00499,
                0x520f74773e74c8c3,
                0x484c8fc982008f0,
                0xee2c3d922008cc6,
            ])).unwrap(),
            infinity: false,
        };

        // Expected
        // x = 52901198670373960614757979459866672334163627229195745167587898707663026648445040826329033206551534205133090753192
        // y = 1711275103908443722918766889652776216989264073722543507596490456144926139887096946237734327757134898380852225872709
        let c = G1Affine {
            x: Fq::from_repr(FqRepr([
                0xef4f05bdd10c8aa8,
                0xad5bf87341a2df9,
                0x81c7424206b78714,
                0x9676ff02ec39c227,
                0x4c12c15d7e55b9f3,
                0x57fd1e317db9bd,
            ])).unwrap(),
            y: Fq::from_repr(FqRepr([
                0x1288334016679345,
                0xf955cd68615ff0b5,
                0xa6998dbaa600f18a,
                0x1267d70db51049fb,
                0x4696deb9ab2ba3e7,
                0xb1e4e11177f59d4,
            ])).unwrap(),
            infinity: false,
        };

        assert!(a.is_on_curve() && a.is_in_correct_subgroup_assuming_on_curve());
        assert!(b.is_on_curve() && b.is_in_correct_subgroup_assuming_on_curve());
        assert!(c.is_on_curve() && c.is_in_correct_subgroup_assuming_on_curve());

        let mut tmp1 = a.into_projective();
        tmp1.add_assign(&b.into_projective());
        assert_eq!(tmp1.into_affine(), c);
        assert_eq!(tmp1, c.into_projective());

        let mut tmp2 = a.into_projective();
        tmp2.add_assign_mixed(&b);
        assert_eq!(tmp2.into_affine(), c);
        assert_eq!(tmp2, c.into_projective());
    }

    #[test]
    fn g1_curve_tests() {
        ::tests::curve::curve_tests::<G1>();
        ::tests::curve::random_transformation_tests_with_cofactor::<G1>();
    }
}

pub mod g2 {
    use super::super::{Bls12, Fq, Fq12, Fq2, FqRepr, Fr, FrRepr};
    use super::g1::G1Affine;
    use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr, SqrtField};
    use rand::{Rand, Rng};
    use std::fmt;
    use {CurveAffine, CurveProjective, EncodedPoint, Engine, GroupDecodingError};

    curve_impl!(
        "G2",
        G2,
        G2Affine,
        G2Prepared,
        Fq2,
        Fr,
        G2Uncompressed,
        G2Compressed,
        G1Affine
    );

    #[derive(Copy, Clone)]
    pub struct G2Uncompressed([u8; 192]);

    impl AsRef<[u8]> for G2Uncompressed {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    impl AsMut<[u8]> for G2Uncompressed {
        fn as_mut(&mut self) -> &mut [u8] {
            &mut self.0
        }
    }

    impl fmt::Debug for G2Uncompressed {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            self.0[..].fmt(formatter)
        }
    }

    impl EncodedPoint for G2Uncompressed {
        type Affine = G2Affine;

        fn empty() -> Self {
            G2Uncompressed([0; 192])
        }
        fn size() -> usize {
            192
        }
        fn into_affine(&self) -> Result<G2Affine, GroupDecodingError> {
            let affine = self.into_affine_unchecked()?;

            if !affine.is_on_curve() {
                Err(GroupDecodingError::NotOnCurve)
            } else if !affine.is_in_correct_subgroup_assuming_on_curve() {
                Err(GroupDecodingError::NotInSubgroup)
            } else {
                Ok(affine)
            }
        }
        fn into_affine_unchecked(&self) -> Result<G2Affine, GroupDecodingError> {
            // Create a copy of this representation.
            let mut copy = self.0;

            if copy[0] & (1 << 7) != 0 {
                // Distinguisher bit is set, but this should be uncompressed!
                return Err(GroupDecodingError::UnexpectedCompressionMode);
            }

            if copy[0] & (1 << 6) != 0 {
                // This is the point at infinity, which means that if we mask away
                // the first two bits, the entire representation should consist
                // of zeroes.
                copy[0] &= 0x3f;

                if copy.iter().all(|b| *b == 0) {
                    Ok(G2Affine::zero())
                } else {
                    Err(GroupDecodingError::UnexpectedInformation)
                }
            } else {
                if copy[0] & (1 << 5) != 0 {
                    // The bit indicating the y-coordinate should be lexicographically
                    // largest is set, but this is an uncompressed element.
                    return Err(GroupDecodingError::UnexpectedInformation);
                }

                // Unset the three most significant bits.
                copy[0] &= 0x1f;

                let mut x_c0 = FqRepr([0; 6]);
                let mut x_c1 = FqRepr([0; 6]);
                let mut y_c0 = FqRepr([0; 6]);
                let mut y_c1 = FqRepr([0; 6]);

                {
                    let mut reader = &copy[..];

                    x_c1.read_be(&mut reader).unwrap();
                    x_c0.read_be(&mut reader).unwrap();
                    y_c1.read_be(&mut reader).unwrap();
                    y_c0.read_be(&mut reader).unwrap();
                }

                Ok(G2Affine {
                    x: Fq2 {
                        c0: Fq::from_repr(x_c0).map_err(|e| {
                            GroupDecodingError::CoordinateDecodingError("x coordinate (c0)", e)
                        })?,
                        c1: Fq::from_repr(x_c1).map_err(|e| {
                            GroupDecodingError::CoordinateDecodingError("x coordinate (c1)", e)
                        })?,
                    },
                    y: Fq2 {
                        c0: Fq::from_repr(y_c0).map_err(|e| {
                            GroupDecodingError::CoordinateDecodingError("y coordinate (c0)", e)
                        })?,
                        c1: Fq::from_repr(y_c1).map_err(|e| {
                            GroupDecodingError::CoordinateDecodingError("y coordinate (c1)", e)
                        })?,
                    },
                    infinity: false,
                })
            }
        }
        fn from_affine(affine: G2Affine) -> Self {
            let mut res = Self::empty();

            if affine.is_zero() {
                // Set the second-most significant bit to indicate this point
                // is at infinity.
                res.0[0] |= 1 << 6;
            } else {
                let mut writer = &mut res.0[..];

                affine.x.c1.into_repr().write_be(&mut writer).unwrap();
                affine.x.c0.into_repr().write_be(&mut writer).unwrap();
                affine.y.c1.into_repr().write_be(&mut writer).unwrap();
                affine.y.c0.into_repr().write_be(&mut writer).unwrap();
            }

            res
        }
    }

    #[derive(Copy, Clone)]
    pub struct G2Compressed([u8; 96]);

    impl AsRef<[u8]> for G2Compressed {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    impl AsMut<[u8]> for G2Compressed {
        fn as_mut(&mut self) -> &mut [u8] {
            &mut self.0
        }
    }

    impl fmt::Debug for G2Compressed {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            self.0[..].fmt(formatter)
        }
    }

    impl EncodedPoint for G2Compressed {
        type Affine = G2Affine;

        fn empty() -> Self {
            G2Compressed([0; 96])
        }
        fn size() -> usize {
            96
        }
        fn into_affine(&self) -> Result<G2Affine, GroupDecodingError> {
            let affine = self.into_affine_unchecked()?;

            // NB: Decompression guarantees that it is on the curve already.

            if !affine.is_in_correct_subgroup_assuming_on_curve() {
                Err(GroupDecodingError::NotInSubgroup)
            } else {
                Ok(affine)
            }
        }
        fn into_affine_unchecked(&self) -> Result<G2Affine, GroupDecodingError> {
            // Create a copy of this representation.
            let mut copy = self.0;

            if copy[0] & (1 << 7) == 0 {
                // Distinguisher bit isn't set.
                return Err(GroupDecodingError::UnexpectedCompressionMode);
            }

            if copy[0] & (1 << 6) != 0 {
                // This is the point at infinity, which means that if we mask away
                // the first two bits, the entire representation should consist
                // of zeroes.
                copy[0] &= 0x3f;

                if copy.iter().all(|b| *b == 0) {
                    Ok(G2Affine::zero())
                } else {
                    Err(GroupDecodingError::UnexpectedInformation)
                }
            } else {
                // Determine if the intended y coordinate must be greater
                // lexicographically.
                let greatest = copy[0] & (1 << 5) != 0;

                // Unset the three most significant bits.
                copy[0] &= 0x1f;

                let mut x_c1 = FqRepr([0; 6]);
                let mut x_c0 = FqRepr([0; 6]);

                {
                    let mut reader = &copy[..];

                    x_c1.read_be(&mut reader).unwrap();
                    x_c0.read_be(&mut reader).unwrap();
                }

                // Interpret as Fq element.
                let x = Fq2 {
                    c0: Fq::from_repr(x_c0).map_err(|e| {
                        GroupDecodingError::CoordinateDecodingError("x coordinate (c0)", e)
                    })?,
                    c1: Fq::from_repr(x_c1).map_err(|e| {
                        GroupDecodingError::CoordinateDecodingError("x coordinate (c1)", e)
                    })?,
                };

                G2Affine::get_point_from_x(x, greatest).ok_or(GroupDecodingError::NotOnCurve)
            }
        }
        fn from_affine(affine: G2Affine) -> Self {
            let mut res = Self::empty();

            if affine.is_zero() {
                // Set the second-most significant bit to indicate this point
                // is at infinity.
                res.0[0] |= 1 << 6;
            } else {
                {
                    let mut writer = &mut res.0[..];

                    affine.x.c1.into_repr().write_be(&mut writer).unwrap();
                    affine.x.c0.into_repr().write_be(&mut writer).unwrap();
                }

                let mut negy = affine.y;
                negy.negate();

                // Set the third most significant bit if the correct y-coordinate
                // is lexicographically largest.
                if affine.y > negy {
                    res.0[0] |= 1 << 5;
                }
            }

            // Set highest bit to distinguish this as a compressed element.
            res.0[0] |= 1 << 7;

            res
        }
    }

    impl G2Affine {
        fn get_generator() -> Self {
            G2Affine {
                x: Fq2 {
                    c0: super::super::fq::G2_GENERATOR_X_C0,
                    c1: super::super::fq::G2_GENERATOR_X_C1,
                },
                y: Fq2 {
                    c0: super::super::fq::G2_GENERATOR_Y_C0,
                    c1: super::super::fq::G2_GENERATOR_Y_C1,
                },
                infinity: false,
            }
        }

        fn get_coeff_b() -> Fq2 {
            Fq2 {
                c0: super::super::fq::B_COEFF,
                c1: super::super::fq::B_COEFF,
            }
        }

        fn scale_by_cofactor(&self) -> G2 {
            // G2 cofactor = (x^8 - 4 x^7 + 5 x^6) - (4 x^4 + 6 x^3 - 4 x^2 - 4 x + 13) // 9
            // 0x5d543a95414e7f1091d50792876a202cd91de4547085abaa68a205b2e5a7ddfa628f1cb4d9e82ef21537e293a6691ae1616ec6e786f0c70cf1c38e31c7238e5
            let cofactor = BitIterator::new([
                0xcf1c38e31c7238e5,
                0x1616ec6e786f0c70,
                0x21537e293a6691ae,
                0xa628f1cb4d9e82ef,
                0xa68a205b2e5a7ddf,
                0xcd91de4547085aba,
                0x91d50792876a202,
                0x5d543a95414e7f1,
            ]);
            self.mul_bits(cofactor)
        }

        fn perform_pairing(&self, other: &G1Affine) -> Fq12 {
            super::super::Bls12::pairing(*other, *self)
        }
    }

    impl G2 {
        fn empirical_recommended_wnaf_for_scalar(scalar: FrRepr) -> usize {
            let num_bits = scalar.num_bits() as usize;

            if num_bits >= 103 {
                4
            } else if num_bits >= 37 {
                3
            } else {
                2
            }
        }

        fn empirical_recommended_wnaf_for_num_scalars(num_scalars: usize) -> usize {
            const RECOMMENDATIONS: [usize; 11] =
                [1, 3, 8, 20, 47, 126, 260, 826, 1501, 4555, 84071];

            let mut ret = 4;
            for r in &RECOMMENDATIONS {
                if num_scalars > *r {
                    ret += 1;
                } else {
                    break;
                }
            }

            ret
        }
    }

    #[derive(Clone, Debug)]
    pub struct G2Prepared {
        pub(crate) coeffs: Vec<(Fq2, Fq2, Fq2)>,
        pub(crate) infinity: bool,
    }

    #[test]
    fn g2_generator() {
        use SqrtField;

        let mut x = Fq2::zero();
        let mut i = 0;
        loop {
            // y^2 = x^3 + b
            let mut rhs = x;
            rhs.square();
            rhs.mul_assign(&x);
            rhs.add_assign(&G2Affine::get_coeff_b());

            if let Some(y) = rhs.sqrt() {
                let mut negy = y;
                negy.negate();

                let p = G2Affine {
                    x: x,
                    y: if y < negy { y } else { negy },
                    infinity: false,
                };

                assert!(!p.is_in_correct_subgroup_assuming_on_curve());

                let g2 = p.scale_by_cofactor();
                if !g2.is_zero() {
                    assert_eq!(i, 2);
                    let g2 = G2Affine::from(g2);

                    assert!(g2.is_in_correct_subgroup_assuming_on_curve());
                    assert_eq!(g2, G2Affine::one());
                    break;
                }
            }

            i += 1;
            x.add_assign(&Fq2::one());
        }
    }

    #[test]
    fn g2_test_is_valid() {
        // Reject point on isomorphic twist (b = 3 * (u + 1))
        {
            let p = G2Affine {
                x: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0xa757072d9fa35ba9,
                        0xae3fb2fb418f6e8a,
                        0xc1598ec46faa0c7c,
                        0x7a17a004747e3dbe,
                        0xcc65406a7c2e5a73,
                        0x10b8c03d64db4d0c,
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0xd30e70fe2f029778,
                        0xda30772df0f5212e,
                        0x5b47a9ff9a233a50,
                        0xfb777e5b9b568608,
                        0x789bac1fec71a2b9,
                        0x1342f02e2da54405,
                    ])).unwrap(),
                },
                y: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0xfe0812043de54dca,
                        0xe455171a3d47a646,
                        0xa493f36bc20be98a,
                        0x663015d9410eb608,
                        0x78e82a79d829a544,
                        0x40a00545bb3c1e,
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0x4709802348e79377,
                        0xb5ac4dc9204bcfbd,
                        0xda361c97d02f42b2,
                        0x15008b1dc399e8df,
                        0x68128fd0548a3829,
                        0x16a613db5c873aaa,
                    ])).unwrap(),
                },
                infinity: false,
            };
            assert!(!p.is_on_curve());
            assert!(p.is_in_correct_subgroup_assuming_on_curve());
        }

        // Reject point on a twist (b = 2 * (u + 1))
        {
            let p = G2Affine {
                x: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0xf4fdfe95a705f917,
                        0xc2914df688233238,
                        0x37c6b12cca35a34b,
                        0x41abba710d6c692c,
                        0xffcc4b2b62ce8484,
                        0x6993ec01b8934ed,
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0xb94e92d5f874e26,
                        0x44516408bc115d95,
                        0xe93946b290caa591,
                        0xa5a0c2b7131f3555,
                        0x83800965822367e7,
                        0x10cf1d3ad8d90bfa,
                    ])).unwrap(),
                },
                y: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0xbf00334c79701d97,
                        0x4fe714f9ff204f9a,
                        0xab70b28002f3d825,
                        0x5a9171720e73eb51,
                        0x38eb4fd8d658adb7,
                        0xb649051bbc1164d,
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0x9225814253d7df75,
                        0xc196c2513477f887,
                        0xe05e2fbd15a804e0,
                        0x55f2b8efad953e04,
                        0x7379345eda55265e,
                        0x377f2e6208fd4cb,
                    ])).unwrap(),
                },
                infinity: false,
            };
            assert!(!p.is_on_curve());
            assert!(!p.is_in_correct_subgroup_assuming_on_curve());
        }

        // Reject point in an invalid subgroup
        // There is only one r-order subgroup, as r does not divide the cofactor.
        {
            let p = G2Affine {
                x: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0x262cea73ea1906c,
                        0x2f08540770fabd6,
                        0x4ceb92d0a76057be,
                        0x2199bc19c48c393d,
                        0x4a151b732a6075bf,
                        0x17762a3b9108c4a7,
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0x26f461e944bbd3d1,
                        0x298f3189a9cf6ed6,
                        0x74328ad8bc2aa150,
                        0x7e147f3f9e6e241,
                        0x72a9b63583963fff,
                        0x158b0083c000462,
                    ])).unwrap(),
                },
                y: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0x91fb0b225ecf103b,
                        0x55d42edc1dc46ba0,
                        0x43939b11997b1943,
                        0x68cad19430706b4d,
                        0x3ccfb97b924dcea8,
                        0x1660f93434588f8d,
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0xaaed3985b6dcb9c7,
                        0xc1e985d6d898d9f4,
                        0x618bd2ac3271ac42,
                        0x3940a2dbb914b529,
                        0xbeb88137cf34f3e7,
                        0x1699ee577c61b694,
                    ])).unwrap(),
                },
                infinity: false,
            };
            assert!(p.is_on_curve());
            assert!(!p.is_in_correct_subgroup_assuming_on_curve());
        }
    }

    #[test]
    fn test_g2_addition_correctness() {
        let mut p = G2 {
            x: Fq2 {
                c0: Fq::from_repr(FqRepr([
                    0x6c994cc1e303094e,
                    0xf034642d2c9e85bd,
                    0x275094f1352123a9,
                    0x72556c999f3707ac,
                    0x4617f2e6774e9711,
                    0x100b2fe5bffe030b,
                ])).unwrap(),
                c1: Fq::from_repr(FqRepr([
                    0x7a33555977ec608,
                    0xe23039d1fe9c0881,
                    0x19ce4678aed4fcb5,
                    0x4637c4f417667e2e,
                    0x93ebe7c3e41f6acc,
                    0xde884f89a9a371b,
                ])).unwrap(),
            },
            y: Fq2 {
                c0: Fq::from_repr(FqRepr([
                    0xe073119472e1eb62,
                    0x44fb3391fe3c9c30,
                    0xaa9b066d74694006,
                    0x25fd427b4122f231,
                    0xd83112aace35cae,
                    0x191b2432407cbb7f,
                ])).unwrap(),
                c1: Fq::from_repr(FqRepr([
                    0xf68ae82fe97662f5,
                    0xe986057068b50b7d,
                    0x96c30f0411590b48,
                    0x9eaa6d19de569196,
                    0xf6a03d31e2ec2183,
                    0x3bdafaf7ca9b39b,
                ])).unwrap(),
            },
            z: Fq2::one(),
        };

        p.add_assign(&G2 {
            x: Fq2 {
                c0: Fq::from_repr(FqRepr([
                    0xa8c763d25910bdd3,
                    0x408777b30ca3add4,
                    0x6115fcc12e2769e,
                    0x8e73a96b329ad190,
                    0x27c546f75ee1f3ab,
                    0xa33d27add5e7e82,
                ])).unwrap(),
                c1: Fq::from_repr(FqRepr([
                    0x93b1ebcd54870dfe,
                    0xf1578300e1342e11,
                    0x8270dca3a912407b,
                    0x2089faf462438296,
                    0x828e5848cd48ea66,
                    0x141ecbac1deb038b,
                ])).unwrap(),
            },
            y: Fq2 {
                c0: Fq::from_repr(FqRepr([
                    0xf5d2c28857229c3f,
                    0x8c1574228757ca23,
                    0xe8d8102175f5dc19,
                    0x2767032fc37cc31d,
                    0xd5ee2aba84fd10fe,
                    0x16576ccd3dd0a4e8,
                ])).unwrap(),
                c1: Fq::from_repr(FqRepr([
                    0x4da9b6f6a96d1dd2,
                    0x9657f7da77f1650e,
                    0xbc150712f9ffe6da,
                    0x31898db63f87363a,
                    0xabab040ddbd097cc,
                    0x11ad236b9ba02990,
                ])).unwrap(),
            },
            z: Fq2::one(),
        });

        let p = G2Affine::from(p);

        assert_eq!(
            p,
            G2Affine {
                x: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0xcde7ee8a3f2ac8af,
                        0xfc642eb35975b069,
                        0xa7de72b7dd0e64b7,
                        0xf1273e6406eef9cc,
                        0xababd760ff05cb92,
                        0xd7c20456617e89
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0xd1a50b8572cbd2b8,
                        0x238f0ac6119d07df,
                        0x4dbe924fe5fd6ac2,
                        0x8b203284c51edf6b,
                        0xc8a0b730bbb21f5e,
                        0x1a3b59d29a31274
                    ])).unwrap(),
                },
                y: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0x9e709e78a8eaa4c9,
                        0xd30921c93ec342f4,
                        0x6d1ef332486f5e34,
                        0x64528ab3863633dc,
                        0x159384333d7cba97,
                        0x4cb84741f3cafe8
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0x242af0dc3640e1a4,
                        0xe90a73ad65c66919,
                        0x2bd7ca7f4346f9ec,
                        0x38528f92b689644d,
                        0xb6884deec59fb21f,
                        0x3c075d3ec52ba90
                    ])).unwrap(),
                },
                infinity: false,
            }
        );
    }

    #[test]
    fn test_g2_doubling_correctness() {
        let mut p = G2 {
            x: Fq2 {
                c0: Fq::from_repr(FqRepr([
                    0x6c994cc1e303094e,
                    0xf034642d2c9e85bd,
                    0x275094f1352123a9,
                    0x72556c999f3707ac,
                    0x4617f2e6774e9711,
                    0x100b2fe5bffe030b,
                ])).unwrap(),
                c1: Fq::from_repr(FqRepr([
                    0x7a33555977ec608,
                    0xe23039d1fe9c0881,
                    0x19ce4678aed4fcb5,
                    0x4637c4f417667e2e,
                    0x93ebe7c3e41f6acc,
                    0xde884f89a9a371b,
                ])).unwrap(),
            },
            y: Fq2 {
                c0: Fq::from_repr(FqRepr([
                    0xe073119472e1eb62,
                    0x44fb3391fe3c9c30,
                    0xaa9b066d74694006,
                    0x25fd427b4122f231,
                    0xd83112aace35cae,
                    0x191b2432407cbb7f,
                ])).unwrap(),
                c1: Fq::from_repr(FqRepr([
                    0xf68ae82fe97662f5,
                    0xe986057068b50b7d,
                    0x96c30f0411590b48,
                    0x9eaa6d19de569196,
                    0xf6a03d31e2ec2183,
                    0x3bdafaf7ca9b39b,
                ])).unwrap(),
            },
            z: Fq2::one(),
        };

        p.double();

        let p = G2Affine::from(p);

        assert_eq!(
            p,
            G2Affine {
                x: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0x91ccb1292727c404,
                        0x91a6cb182438fad7,
                        0x116aee59434de902,
                        0xbcedcfce1e52d986,
                        0x9755d4a3926e9862,
                        0x18bab73760fd8024
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0x4e7c5e0a2ae5b99e,
                        0x96e582a27f028961,
                        0xc74d1cf4ef2d5926,
                        0xeb0cf5e610ef4fe7,
                        0x7b4c2bae8db6e70b,
                        0xf136e43909fca0
                    ])).unwrap(),
                },
                y: Fq2 {
                    c0: Fq::from_repr(FqRepr([
                        0x954d4466ab13e58,
                        0x3ee42eec614cf890,
                        0x853bb1d28877577e,
                        0xa5a2a51f7fde787b,
                        0x8b92866bc6384188,
                        0x81a53fe531d64ef
                    ])).unwrap(),
                    c1: Fq::from_repr(FqRepr([
                        0x4c5d607666239b34,
                        0xeddb5f48304d14b3,
                        0x337167ee6e8e3cb6,
                        0xb271f52f12ead742,
                        0x244e6c2015c83348,
                        0x19e2deae6eb9b441
                    ])).unwrap(),
                },
                infinity: false,
            }
        );
    }

    #[test]
    fn g2_curve_tests() {
        ::tests::curve::curve_tests::<G2>();
        ::tests::curve::random_transformation_tests_with_cofactor::<G2>();
    }
}

pub use self::g1::*;
pub use self::g2::*;
