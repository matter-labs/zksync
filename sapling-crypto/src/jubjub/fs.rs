use byteorder::{ByteOrder, LittleEndian};
use ff::{adc, sbb, mac_with_carry};
use ff::{BitIterator, Field, PrimeField, SqrtField, PrimeFieldRepr, PrimeFieldDecodingError, LegendreSymbol};
use ff::LegendreSymbol::*;
use super::ToUniform;

// s = 6554484396890773809930967563523245729705921265872317281365359162392183254199
const MODULUS: FsRepr = FsRepr([0xd0970e5ed6f72cb7, 0xa6682093ccc81082, 0x6673b0101343b00, 0xe7db4ea6533afa9]);

// The number of bits needed to represent the modulus.
const MODULUS_BITS: u32 = 252;

// The number of bits that must be shaved from the beginning of
// the representation when randomly sampling.
const REPR_SHAVE_BITS: u32 = 4;

// R = 2**256 % s
const R: FsRepr = FsRepr([0x25f80bb3b99607d9, 0xf315d62f66b6e750, 0x932514eeeb8814f4, 0x9a6fc6f479155c6]);

// R2 = R^2 % s
const R2: FsRepr = FsRepr([0x67719aa495e57731, 0x51b0cef09ce3fc26, 0x69dab7fac026e9a5, 0x4f6547b8d127688]);

// INV = -(s^{-1} mod 2^64) mod s
const INV: u64 = 0x1ba3a358ef788ef9;

// GENERATOR = 6 (multiplicative generator of r-1 order, that is also quadratic nonresidue)
const GENERATOR: FsRepr = FsRepr([0x720b1b19d49ea8f1, 0xbf4aa36101f13a58, 0x5fa8cc968193ccbb, 0xe70cbdc7dccf3ac]);

// 2^S * t = MODULUS - 1 with t odd
const S: u32 = 1;

// 2^S root of unity computed by GENERATOR^t
const ROOT_OF_UNITY: FsRepr = FsRepr([0xaa9f02ab1d6124de, 0xb3524a6466112932, 0x7342261215ac260b, 0x4d6b87b1da259e2]);

// -((2**256) mod s) mod s
const NEGATIVE_ONE: Fs = Fs(FsRepr([0xaa9f02ab1d6124de, 0xb3524a6466112932, 0x7342261215ac260b, 0x4d6b87b1da259e2]));

/// This is the underlying representation of an element of `Fs`.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct FsRepr(pub [u64; 4]);

impl ::rand::Rand for FsRepr {
    #[inline(always)]
    fn rand<R: ::rand::Rng>(rng: &mut R) -> Self {
        FsRepr(rng.gen())
    }
}

impl ::std::fmt::Display for FsRepr
{
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        try!(write!(f, "0x"));
        for i in self.0.iter().rev() {
            try!(write!(f, "{:016x}", *i));
        }

        Ok(())
    }
}

impl AsRef<[u64]> for FsRepr {
    #[inline(always)]
    fn as_ref(&self) -> &[u64] {
        &self.0
    }
}

impl AsMut<[u64]> for FsRepr {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u64] {
        &mut self.0
    }
}

impl From<u64> for FsRepr {
    #[inline(always)]
    fn from(val: u64) -> FsRepr {
        let mut repr = Self::default();
        repr.0[0] = val;
        repr
    }
}

impl Ord for FsRepr {
    #[inline(always)]
    fn cmp(&self, other: &FsRepr) -> ::std::cmp::Ordering {
        for (a, b) in self.0.iter().rev().zip(other.0.iter().rev()) {
            if a < b {
                return ::std::cmp::Ordering::Less
            } else if a > b {
                return ::std::cmp::Ordering::Greater
            }
        }

        ::std::cmp::Ordering::Equal
    }
}

impl PartialOrd for FsRepr {
    #[inline(always)]
    fn partial_cmp(&self, other: &FsRepr) -> Option<::std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PrimeFieldRepr for FsRepr {
    #[inline(always)]
    fn is_odd(&self) -> bool {
        self.0[0] & 1 == 1
    }

    #[inline(always)]
    fn is_even(&self) -> bool {
        !self.is_odd()
    }

    #[inline(always)]
    fn is_zero(&self) -> bool {
        self.0.iter().all(|&e| e == 0)
    }

    #[inline(always)]
    fn shr(&mut self, mut n: u32) {
        if n >= 64 * 4 {
            *self = Self::from(0);
            return;
        }

        while n >= 64 {
            let mut t = 0;
            for i in self.0.iter_mut().rev() {
                ::std::mem::swap(&mut t, i);
            }
            n -= 64;
        }

        if n > 0 {
            let mut t = 0;
            for i in self.0.iter_mut().rev() {
                let t2 = *i << (64 - n);
                *i >>= n;
                *i |= t;
                t = t2;
            }
        }
    }

    #[inline(always)]
    fn div2(&mut self) {
        let mut t = 0;
        for i in self.0.iter_mut().rev() {
            let t2 = *i << 63;
            *i >>= 1;
            *i |= t;
            t = t2;
        }
    }

    #[inline(always)]
    fn mul2(&mut self) {
        let mut last = 0;
        for i in &mut self.0 {
            let tmp = *i >> 63;
            *i <<= 1;
            *i |= last;
            last = tmp;
        }
    }

    #[inline(always)]
    fn shl(&mut self, mut n: u32) {
        if n >= 64 * 4 {
            *self = Self::from(0);
            return;
        }

        while n >= 64 {
            let mut t = 0;
            for i in &mut self.0 {
                ::std::mem::swap(&mut t, i);
            }
            n -= 64;
        }

        if n > 0 {
            let mut t = 0;
            for i in &mut self.0 {
                let t2 = *i >> (64 - n);
                *i <<= n;
                *i |= t;
                t = t2;
            }
        }
    }

    #[inline(always)]
    fn num_bits(&self) -> u32 {
        let mut ret = (4 as u32) * 64;
        for i in self.0.iter().rev() {
            let leading = i.leading_zeros();
            ret -= leading;
            if leading != 64 {
                break;
            }
        }

        ret
    }

    #[inline(always)]
    fn add_nocarry(&mut self, other: &FsRepr) {
        let mut carry = 0;

        for (a, b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = adc(*a, *b, &mut carry);
        }
    }

    #[inline(always)]
    fn sub_noborrow(&mut self, other: &FsRepr) {
        let mut borrow = 0;

        for (a, b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = sbb(*a, *b, &mut borrow);
        }
    }
}

/// This is an element of the scalar field of the Jubjub curve.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fs(FsRepr);

impl ::std::fmt::Display for Fs
{
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "Fs({})", self.into_repr())
    }
}

impl ::rand::Rand for Fs {
    fn rand<R: ::rand::Rng>(rng: &mut R) -> Self {
        loop {
            let mut tmp = Fs(FsRepr::rand(rng));

            // Mask away the unused bits at the beginning.
            tmp.0.as_mut()[3] &= 0xffffffffffffffff >> REPR_SHAVE_BITS;

            if tmp.is_valid() {
                return tmp
            }
        }
    }
}

impl From<Fs> for FsRepr {
    fn from(e: Fs) -> FsRepr {
        e.into_repr()
    }
}

impl PrimeField for Fs {
    type Repr = FsRepr;

    fn from_repr(r: FsRepr) -> Result<Fs, PrimeFieldDecodingError> {
        let mut r = Fs(r);
        if r.is_valid() {
            r.mul_assign(&Fs(R2));

            Ok(r)
        } else {
            Err(PrimeFieldDecodingError::NotInField(format!("{}", r.0)))
        }
    }

    fn into_repr(&self) -> FsRepr {
        let mut r = *self;
        r.mont_reduce((self.0).0[0], (self.0).0[1],
                      (self.0).0[2], (self.0).0[3],
                      0, 0, 0, 0);
        r.0
    }

    fn char() -> FsRepr {
        MODULUS
    }

    const NUM_BITS: u32 = MODULUS_BITS;

    const CAPACITY: u32 = Self::NUM_BITS - 1;

    fn multiplicative_generator() -> Self {
        Fs(GENERATOR)
    }

    const S: u32 = S;

    fn root_of_unity() -> Self {
        Fs(ROOT_OF_UNITY)
    }
}

impl Field for Fs {
    #[inline]
    fn zero() -> Self {
        Fs(FsRepr::from(0))
    }

    #[inline]
    fn one() -> Self {
        Fs(R)
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[inline]
    fn add_assign(&mut self, other: &Fs) {
        // This cannot exceed the backing capacity.
        self.0.add_nocarry(&other.0);

        // However, it may need to be reduced.
        self.reduce();
    }

    #[inline]
    fn double(&mut self) {
        // This cannot exceed the backing capacity.
        self.0.mul2();

        // However, it may need to be reduced.
        self.reduce();
    }

    #[inline]
    fn sub_assign(&mut self, other: &Fs) {
        // If `other` is larger than `self`, we'll need to add the modulus to self first.
        if other.0 > self.0 {
            self.0.add_nocarry(&MODULUS);
        }

        self.0.sub_noborrow(&other.0);
    }

    #[inline]
    fn negate(&mut self) {
        if !self.is_zero() {
            let mut tmp = MODULUS;
            tmp.sub_noborrow(&self.0);
            self.0 = tmp;
        }
    }

    fn inverse(&self) -> Option<Self> {
        if self.is_zero() {
            None
        } else {
            // Guajardo Kumar Paar Pelzl
            // Efficient Software-Implementation of Finite Fields with Applications to Cryptography
            // Algorithm 16 (BEA for Inversion in Fp)

            let one = FsRepr::from(1);

            let mut u = self.0;
            let mut v = MODULUS;
            let mut b = Fs(R2); // Avoids unnecessary reduction step.
            let mut c = Self::zero();

            while u != one && v != one {
                while u.is_even() {
                    u.div2();

                    if b.0.is_even() {
                        b.0.div2();
                    } else {
                        b.0.add_nocarry(&MODULUS);
                        b.0.div2();
                    }
                }

                while v.is_even() {
                    v.div2();

                    if c.0.is_even() {
                        c.0.div2();
                    } else {
                        c.0.add_nocarry(&MODULUS);
                        c.0.div2();
                    }
                }

                if v < u {
                    u.sub_noborrow(&v);
                    b.sub_assign(&c);
                } else {
                    v.sub_noborrow(&u);
                    c.sub_assign(&b);
                }
            }

            if u == one {
                Some(b)
            } else {
                Some(c)
            }
        }
    }

    #[inline(always)]
    fn frobenius_map(&mut self, _: usize) {
        // This has no effect in a prime field.
    }

    #[inline]
    fn mul_assign(&mut self, other: &Fs)
    {
        let mut carry = 0;
        let r0 = mac_with_carry(0, (self.0).0[0], (other.0).0[0], &mut carry);
        let r1 = mac_with_carry(0, (self.0).0[0], (other.0).0[1], &mut carry);
        let r2 = mac_with_carry(0, (self.0).0[0], (other.0).0[2], &mut carry);
        let r3 = mac_with_carry(0, (self.0).0[0], (other.0).0[3], &mut carry);
        let r4 = carry;
        let mut carry = 0;
        let r1 = mac_with_carry(r1, (self.0).0[1], (other.0).0[0], &mut carry);
        let r2 = mac_with_carry(r2, (self.0).0[1], (other.0).0[1], &mut carry);
        let r3 = mac_with_carry(r3, (self.0).0[1], (other.0).0[2], &mut carry);
        let r4 = mac_with_carry(r4, (self.0).0[1], (other.0).0[3], &mut carry);
        let r5 = carry;
        let mut carry = 0;
        let r2 = mac_with_carry(r2, (self.0).0[2], (other.0).0[0], &mut carry);
        let r3 = mac_with_carry(r3, (self.0).0[2], (other.0).0[1], &mut carry);
        let r4 = mac_with_carry(r4, (self.0).0[2], (other.0).0[2], &mut carry);
        let r5 = mac_with_carry(r5, (self.0).0[2], (other.0).0[3], &mut carry);
        let r6 = carry;
        let mut carry = 0;
        let r3 = mac_with_carry(r3, (self.0).0[3], (other.0).0[0], &mut carry);
        let r4 = mac_with_carry(r4, (self.0).0[3], (other.0).0[1], &mut carry);
        let r5 = mac_with_carry(r5, (self.0).0[3], (other.0).0[2], &mut carry);
        let r6 = mac_with_carry(r6, (self.0).0[3], (other.0).0[3], &mut carry);
        let r7 = carry;
        self.mont_reduce(r0, r1, r2, r3, r4, r5, r6, r7);
    }

    #[inline]
    fn square(&mut self)
    {
        let mut carry = 0;
        let r1 = mac_with_carry(0, (self.0).0[0], (self.0).0[1], &mut carry);
        let r2 = mac_with_carry(0, (self.0).0[0], (self.0).0[2], &mut carry);
        let r3 = mac_with_carry(0, (self.0).0[0], (self.0).0[3], &mut carry);
        let r4 = carry;
        let mut carry = 0;
        let r3 = mac_with_carry(r3, (self.0).0[1], (self.0).0[2], &mut carry);
        let r4 = mac_with_carry(r4, (self.0).0[1], (self.0).0[3], &mut carry);
        let r5 = carry;
        let mut carry = 0;
        let r5 = mac_with_carry(r5, (self.0).0[2], (self.0).0[3], &mut carry);
        let r6 = carry;

        let r7 = r6 >> 63;
        let r6 = (r6 << 1) | (r5 >> 63);
        let r5 = (r5 << 1) | (r4 >> 63);
        let r4 = (r4 << 1) | (r3 >> 63);
        let r3 = (r3 << 1) | (r2 >> 63);
        let r2 = (r2 << 1) | (r1 >> 63);
        let r1 = r1 << 1;

        let mut carry = 0;
        let r0 = mac_with_carry(0, (self.0).0[0], (self.0).0[0], &mut carry);
        let r1 = adc(r1, 0, &mut carry);
        let r2 = mac_with_carry(r2, (self.0).0[1], (self.0).0[1], &mut carry);
        let r3 = adc(r3, 0, &mut carry);
        let r4 = mac_with_carry(r4, (self.0).0[2], (self.0).0[2], &mut carry);
        let r5 = adc(r5, 0, &mut carry);
        let r6 = mac_with_carry(r6, (self.0).0[3], (self.0).0[3], &mut carry);
        let r7 = adc(r7, 0, &mut carry);
        self.mont_reduce(r0, r1, r2, r3, r4, r5, r6, r7);
    }
}

impl Fs {
    /// Determines if the element is really in the field. This is only used
    /// internally.
    #[inline(always)]
    fn is_valid(&self) -> bool {
        self.0 < MODULUS
    }

    /// Subtracts the modulus from this element if this element is not in the
    /// field. Only used internally.
    #[inline(always)]
    fn reduce(&mut self) {
        if !self.is_valid() {
            self.0.sub_noborrow(&MODULUS);
        }
    }

    #[inline(always)]
    fn mont_reduce(
        &mut self,
        r0: u64,
        mut r1: u64,
        mut r2: u64,
        mut r3: u64,
        mut r4: u64,
        mut r5: u64,
        mut r6: u64,
        mut r7: u64
    )
    {
        // The Montgomery reduction here is based on Algorithm 14.32 in
        // Handbook of Applied Cryptography
        // <http://cacr.uwaterloo.ca/hac/about/chap14.pdf>.

        let k = r0.wrapping_mul(INV);
        let mut carry = 0;
        mac_with_carry(r0, k, MODULUS.0[0], &mut carry);
        r1 = mac_with_carry(r1, k, MODULUS.0[1], &mut carry);
        r2 = mac_with_carry(r2, k, MODULUS.0[2], &mut carry);
        r3 = mac_with_carry(r3, k, MODULUS.0[3], &mut carry);
        r4 = adc(r4, 0, &mut carry);
        let carry2 = carry;
        let k = r1.wrapping_mul(INV);
        let mut carry = 0;
        mac_with_carry(r1, k, MODULUS.0[0], &mut carry);
        r2 = mac_with_carry(r2, k, MODULUS.0[1], &mut carry);
        r3 = mac_with_carry(r3, k, MODULUS.0[2], &mut carry);
        r4 = mac_with_carry(r4, k, MODULUS.0[3], &mut carry);
        r5 = adc(r5, carry2, &mut carry);
        let carry2 = carry;
        let k = r2.wrapping_mul(INV);
        let mut carry = 0;
        mac_with_carry(r2, k, MODULUS.0[0], &mut carry);
        r3 = mac_with_carry(r3, k, MODULUS.0[1], &mut carry);
        r4 = mac_with_carry(r4, k, MODULUS.0[2], &mut carry);
        r5 = mac_with_carry(r5, k, MODULUS.0[3], &mut carry);
        r6 = adc(r6, carry2, &mut carry);
        let carry2 = carry;
        let k = r3.wrapping_mul(INV);
        let mut carry = 0;
        mac_with_carry(r3, k, MODULUS.0[0], &mut carry);
        r4 = mac_with_carry(r4, k, MODULUS.0[1], &mut carry);
        r5 = mac_with_carry(r5, k, MODULUS.0[2], &mut carry);
        r6 = mac_with_carry(r6, k, MODULUS.0[3], &mut carry);
        r7 = adc(r7, carry2, &mut carry);
        (self.0).0[0] = r4;
        (self.0).0[1] = r5;
        (self.0).0[2] = r6;
        (self.0).0[3] = r7;
        self.reduce();
    }

    fn mul_bits<S: AsRef<[u64]>>(&self, bits: BitIterator<S>) -> Self {
        let mut res = Self::zero();
        for bit in bits {
            res.double();

            if bit {
                res.add_assign(self)
            }
        }
        res
    }
}

impl ToUniform for Fs {
    /// Convert a little endian byte string into a uniform
    /// field element. The number is reduced mod s. The caller
    /// is responsible for ensuring the input is 64 bytes of
    /// Random Oracle output.
    fn to_uniform(digest: &[u8]) -> Self {
        assert_eq!(digest.len(), 64);
        let mut repr: [u64; 8] = [0; 8];
        LittleEndian::read_u64_into(digest, &mut repr);
        Self::one().mul_bits(BitIterator::new(repr))
    }

    /// Convert a little endian byte string into a uniform
    /// field element. The number is reduced mod s. The caller
    /// is responsible for ensuring the input is 32 bytes of
    /// Random Oracle output.
    fn to_uniform_32(digest: &[u8]) -> Self {
        assert_eq!(digest.len(), 32);
        let mut repr: [u64; 4] = [0; 4];
        LittleEndian::read_u64_into(digest, &mut repr);
        Self::one().mul_bits(BitIterator::new(repr))
    }
}

impl SqrtField for Fs {

    fn legendre(&self) -> LegendreSymbol {
        // s = self^((s - 1) // 2)
        let s = self.pow([0x684b872f6b7b965b, 0x53341049e6640841, 0x83339d80809a1d80, 0x73eda753299d7d4]);
        if s == Self::zero() { Zero }
        else if s == Self::one() { QuadraticResidue }
        else { QuadraticNonResidue }
    }

    fn sqrt(&self) -> Option<Self> {
        // Shank's algorithm for s mod 4 = 3
        // https://eprint.iacr.org/2012/685.pdf (page 9, algorithm 2)

        // a1 = self^((s - 3) // 4)
        let mut a1 = self.pow([0xb425c397b5bdcb2d, 0x299a0824f3320420, 0x4199cec0404d0ec0, 0x39f6d3a994cebea]);
        let mut a0 = a1;
        a0.square();
        a0.mul_assign(self);

        if a0 == NEGATIVE_ONE
        {
            None
        }
        else
        {
            a1.mul_assign(self);
            Some(a1)
        }
    }
}


#[test]
fn test_neg_one() {
    let mut o = Fs::one();
    o.negate();

    assert_eq!(NEGATIVE_ONE, o);
}

#[cfg(test)]
use rand::{SeedableRng, XorShiftRng, Rand};

#[test]
fn test_fs_repr_ordering() {
    fn assert_equality(a: FsRepr, b: FsRepr) {
        assert_eq!(a, b);
        assert!(a.cmp(&b) == ::std::cmp::Ordering::Equal);
    }

    fn assert_lt(a: FsRepr, b: FsRepr) {
        assert!(a < b);
        assert!(b > a);
    }

    assert_equality(FsRepr([9999, 9999, 9999, 9999]), FsRepr([9999, 9999, 9999, 9999]));
    assert_equality(FsRepr([9999, 9998, 9999, 9999]), FsRepr([9999, 9998, 9999, 9999]));
    assert_equality(FsRepr([9999, 9999, 9999, 9997]), FsRepr([9999, 9999, 9999, 9997]));
    assert_lt(FsRepr([9999, 9997, 9999, 9998]), FsRepr([9999, 9997, 9999, 9999]));
    assert_lt(FsRepr([9999, 9997, 9998, 9999]), FsRepr([9999, 9997, 9999, 9999]));
    assert_lt(FsRepr([9, 9999, 9999, 9997]), FsRepr([9999, 9999, 9999, 9997]));
}

#[test]
fn test_fs_repr_from() {
    assert_eq!(FsRepr::from(100), FsRepr([100, 0, 0, 0]));
}

#[test]
fn test_fs_repr_is_odd() {
    assert!(!FsRepr::from(0).is_odd());
    assert!(FsRepr::from(0).is_even());
    assert!(FsRepr::from(1).is_odd());
    assert!(!FsRepr::from(1).is_even());
    assert!(!FsRepr::from(324834872).is_odd());
    assert!(FsRepr::from(324834872).is_even());
    assert!(FsRepr::from(324834873).is_odd());
    assert!(!FsRepr::from(324834873).is_even());
}

#[test]
fn test_fs_repr_is_zero() {
    assert!(FsRepr::from(0).is_zero());
    assert!(!FsRepr::from(1).is_zero());
    assert!(!FsRepr([0, 0, 1, 0]).is_zero());
}

#[test]
fn test_fs_repr_div2() {
    let mut a = FsRepr([0xbd2920b19c972321, 0x174ed0466a3be37e, 0xd468d5e3b551f0b5, 0xcb67c072733beefc]);
    a.div2();
    assert_eq!(a, FsRepr([0x5e949058ce4b9190, 0x8ba76823351df1bf, 0x6a346af1daa8f85a, 0x65b3e039399df77e]));
    for _ in 0..10 {
        a.div2();
    }
    assert_eq!(a, FsRepr([0x6fd7a524163392e4, 0x16a2e9da08cd477c, 0xdf9a8d1abc76aa3e, 0x196cf80e4e677d]));
    for _ in 0..200 {
        a.div2();
    }
    assert_eq!(a, FsRepr([0x196cf80e4e67, 0x0, 0x0, 0x0]));
    for _ in 0..40 {
        a.div2();
    }
    assert_eq!(a, FsRepr([0x19, 0x0, 0x0, 0x0]));
    for _ in 0..4 {
        a.div2();
    }
    assert_eq!(a, FsRepr([0x1, 0x0, 0x0, 0x0]));
    a.div2();
    assert!(a.is_zero());
}

#[test]
fn test_fs_repr_shr() {
    let mut a = FsRepr([0xb33fbaec482a283f, 0x997de0d3a88cb3df, 0x9af62d2a9a0e5525, 0x36003ab08de70da1]);
    a.shr(0);
    assert_eq!(
        a,
        FsRepr([0xb33fbaec482a283f, 0x997de0d3a88cb3df, 0x9af62d2a9a0e5525, 0x36003ab08de70da1])
    );
    a.shr(1);
    assert_eq!(
        a,
        FsRepr([0xd99fdd762415141f, 0xccbef069d44659ef, 0xcd7b16954d072a92, 0x1b001d5846f386d0])
    );
    a.shr(50);
    assert_eq!(
        a,
        FsRepr([0xbc1a7511967bf667, 0xc5a55341caa4b32f, 0x75611bce1b4335e, 0x6c0])
    );
    a.shr(130);
    assert_eq!(
        a,
        FsRepr([0x1d5846f386d0cd7, 0x1b0, 0x0, 0x0])
    );
    a.shr(64);
    assert_eq!(
        a,
        FsRepr([0x1b0, 0x0, 0x0, 0x0])
    );
}

#[test]
fn test_fs_repr_mul2() {
    let mut a = FsRepr::from(23712937547);
    a.mul2();
    assert_eq!(a, FsRepr([0xb0acd6c96, 0x0, 0x0, 0x0]));
    for _ in 0..60 {
        a.mul2();
    }
    assert_eq!(a, FsRepr([0x6000000000000000, 0xb0acd6c9, 0x0, 0x0]));
    for _ in 0..128 {
        a.mul2();
    }
    assert_eq!(a, FsRepr([0x0, 0x0, 0x6000000000000000, 0xb0acd6c9]));
    for _ in 0..60 {
        a.mul2();
    }
    assert_eq!(a, FsRepr([0x0, 0x0, 0x0, 0x9600000000000000]));
    for _ in 0..7 {
        a.mul2();
    }
    assert!(a.is_zero());
}

#[test]
fn test_fs_repr_num_bits() {
    let mut a = FsRepr::from(0);
    assert_eq!(0, a.num_bits());
    a = FsRepr::from(1);
    for i in 1..257 {
        assert_eq!(i, a.num_bits());
        a.mul2();
    }
    assert_eq!(0, a.num_bits());
}

#[test]
fn test_fs_repr_sub_noborrow() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let mut t = FsRepr([0x8e62a7e85264e2c3, 0xb23d34c1941d3ca, 0x5976930b7502dd15, 0x600f3fb517bf5495]);
    t.sub_noborrow(&FsRepr([0xd64f669809cbc6a4, 0xfa76cb9d90cf7637, 0xfefb0df9038d43b3, 0x298a30c744b31acf]));
    assert!(t == FsRepr([0xb813415048991c1f, 0x10ad07ae88725d92, 0x5a7b851271759961, 0x36850eedd30c39c5]));

    for _ in 0..1000 {
        let mut a = FsRepr::rand(&mut rng);
        a.0[3] >>= 30;
        let mut b = a;
        for _ in 0..10 {
            b.mul2();
        }
        let mut c = b;
        for _ in 0..10 {
            c.mul2();
        }

        assert!(a < b);
        assert!(b < c);

        let mut csub_ba = c;
        csub_ba.sub_noborrow(&b);
        csub_ba.sub_noborrow(&a);

        let mut csub_ab = c;
        csub_ab.sub_noborrow(&a);
        csub_ab.sub_noborrow(&b);

        assert_eq!(csub_ab, csub_ba);
    }
}

#[test]
fn test_fs_legendre() {
    assert_eq!(QuadraticResidue, Fs::one().legendre());
    assert_eq!(Zero, Fs::zero().legendre());

    let e = FsRepr([0x8385eec23df1f88e, 0x9a01fb412b2dba16, 0x4c928edcdd6c22f, 0x9f2df7ef69ecef9]);
    assert_eq!(QuadraticResidue, Fs::from_repr(e).unwrap().legendre());
    let e = FsRepr([0xe8ed9f299da78568, 0x35efdebc88b2209, 0xc82125cb1f916dbe, 0x6813d2b38c39bd0]);
    assert_eq!(QuadraticNonResidue, Fs::from_repr(e).unwrap().legendre());
}

#[test]
fn test_fr_repr_add_nocarry() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let mut t = FsRepr([0xd64f669809cbc6a4, 0xfa76cb9d90cf7637, 0xfefb0df9038d43b3, 0x298a30c744b31acf]);
    t.add_nocarry(&FsRepr([0x8e62a7e85264e2c3, 0xb23d34c1941d3ca, 0x5976930b7502dd15, 0x600f3fb517bf5495]));
    assert_eq!(t, FsRepr([0x64b20e805c30a967, 0x59a9ee9aa114a02, 0x5871a104789020c9, 0x8999707c5c726f65]));

    // Test for the associativity of addition.
    for _ in 0..1000 {
        let mut a = FsRepr::rand(&mut rng);
        let mut b = FsRepr::rand(&mut rng);
        let mut c = FsRepr::rand(&mut rng);

        // Unset the first few bits, so that overflow won't occur.
        a.0[3] >>= 3;
        b.0[3] >>= 3;
        c.0[3] >>= 3;

        let mut abc = a;
        abc.add_nocarry(&b);
        abc.add_nocarry(&c);

        let mut acb = a;
        acb.add_nocarry(&c);
        acb.add_nocarry(&b);

        let mut bac = b;
        bac.add_nocarry(&a);
        bac.add_nocarry(&c);

        let mut bca = b;
        bca.add_nocarry(&c);
        bca.add_nocarry(&a);

        let mut cab = c;
        cab.add_nocarry(&a);
        cab.add_nocarry(&b);

        let mut cba = c;
        cba.add_nocarry(&b);
        cba.add_nocarry(&a);

        assert_eq!(abc, acb);
        assert_eq!(abc, bac);
        assert_eq!(abc, bca);
        assert_eq!(abc, cab);
        assert_eq!(abc, cba);
    }
}

#[test]
fn test_fs_is_valid() {
    let mut a = Fs(MODULUS);
    assert!(!a.is_valid());
    a.0.sub_noborrow(&FsRepr::from(1));
    assert!(a.is_valid());
    assert!(Fs(FsRepr::from(0)).is_valid());
    assert!(Fs(FsRepr([0xd0970e5ed6f72cb6, 0xa6682093ccc81082, 0x6673b0101343b00, 0xe7db4ea6533afa9])).is_valid());
    assert!(!Fs(FsRepr([0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff])).is_valid());

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let a = Fs::rand(&mut rng);
        assert!(a.is_valid());
    }
}

#[test]
fn test_fs_add_assign() {
    {
        // Random number
        let mut tmp = Fs::from_str("4577408157467272683998459759522778614363623736323078995109579213719612604198").unwrap();
        assert!(tmp.is_valid());
        // Test that adding zero has no effect.
        tmp.add_assign(&Fs(FsRepr::from(0)));
        assert_eq!(tmp, Fs(FsRepr([0x8e6bfff4722d6e67, 0x5643da5c892044f9, 0x9465f4b281921a69, 0x25f752d3edd7162])));
        // Add one and test for the result.
        tmp.add_assign(&Fs(FsRepr::from(1)));
        assert_eq!(tmp, Fs(FsRepr([0x8e6bfff4722d6e68, 0x5643da5c892044f9, 0x9465f4b281921a69, 0x25f752d3edd7162])));
        // Add another random number that exercises the reduction.
        tmp.add_assign(&Fs(FsRepr([0xb634d07bc42d4a70, 0xf724f0c008411f5f, 0x456d4053d865af34, 0x24ce814e8c63027])));
        assert_eq!(tmp, Fs(FsRepr([0x44a0d070365ab8d8, 0x4d68cb1c91616459, 0xd9d3350659f7c99e, 0x4ac5d4227a3a189])));
        // Add one to (s - 1) and test for the result.
        tmp = Fs(FsRepr([0xd0970e5ed6f72cb6, 0xa6682093ccc81082, 0x6673b0101343b00, 0xe7db4ea6533afa9]));
        tmp.add_assign(&Fs(FsRepr::from(1)));
        assert!(tmp.0.is_zero());
        // Add a random number to another one such that the result is s - 1
        tmp = Fs(FsRepr([0xa11fda5950ce3636, 0x922e0dbccfe0ca0e, 0xacebb6e215b82d4a, 0x97ffb8cdc3aee93]));
        tmp.add_assign(&Fs(FsRepr([0x2f7734058628f680, 0x143a12d6fce74674, 0x597b841eeb7c0db6, 0x4fdb95d88f8c115])));
        assert_eq!(tmp, Fs(FsRepr([0xd0970e5ed6f72cb6, 0xa6682093ccc81082, 0x6673b0101343b00, 0xe7db4ea6533afa9])));
        // Add one to the result and test for it.
        tmp.add_assign(&Fs(FsRepr::from(1)));
        assert!(tmp.0.is_zero());
    }

    // Test associativity

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        // Generate a, b, c and ensure (a + b) + c == a + (b + c).
        let a = Fs::rand(&mut rng);
        let b = Fs::rand(&mut rng);
        let c = Fs::rand(&mut rng);

        let mut tmp1 = a;
        tmp1.add_assign(&b);
        tmp1.add_assign(&c);

        let mut tmp2 = b;
        tmp2.add_assign(&c);
        tmp2.add_assign(&a);

        assert!(tmp1.is_valid());
        assert!(tmp2.is_valid());
        assert_eq!(tmp1, tmp2);
    }
}

#[test]
fn test_fs_sub_assign() {
    {
        // Test arbitrary subtraction that tests reduction.
        let mut tmp = Fs(FsRepr([0xb384d9f6877afd99, 0x4442513958e1a1c1, 0x352c4b8a95eccc3f, 0x2db62dee4b0f2]));
        tmp.sub_assign(&Fs(FsRepr([0xec5bd2d13ed6b05a, 0x2adc0ab3a39b5fa, 0x82d3360a493e637e, 0x53ccff4a64d6679])));
        assert_eq!(tmp, Fs(FsRepr([0x97c015841f9b79f6, 0xe7fcb121eb6ffc49, 0xb8c050814de2a3c1, 0x943c0589dcafa21])));

        // Test the opposite subtraction which doesn't test reduction.
        tmp = Fs(FsRepr([0xec5bd2d13ed6b05a, 0x2adc0ab3a39b5fa, 0x82d3360a493e637e, 0x53ccff4a64d6679]));
        tmp.sub_assign(&Fs(FsRepr([0xb384d9f6877afd99, 0x4442513958e1a1c1, 0x352c4b8a95eccc3f, 0x2db62dee4b0f2])));
        assert_eq!(tmp, Fs(FsRepr([0x38d6f8dab75bb2c1, 0xbe6b6f71e1581439, 0x4da6ea7fb351973e, 0x539f491c768b587])));

        // Test for sensible results with zero
        tmp = Fs(FsRepr::from(0));
        tmp.sub_assign(&Fs(FsRepr::from(0)));
        assert!(tmp.is_zero());

        tmp = Fs(FsRepr([0x361e16aef5cce835, 0x55bbde2536e274c1, 0x4dc77a63fd15ee75, 0x1e14bb37c14f230]));
        tmp.sub_assign(&Fs(FsRepr::from(0)));
        assert_eq!(tmp, Fs(FsRepr([0x361e16aef5cce835, 0x55bbde2536e274c1, 0x4dc77a63fd15ee75, 0x1e14bb37c14f230])));
    }

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        // Ensure that (a - b) + (b - a) = 0.
        let a = Fs::rand(&mut rng);
        let b = Fs::rand(&mut rng);

        let mut tmp1 = a;
        tmp1.sub_assign(&b);

        let mut tmp2 = b;
        tmp2.sub_assign(&a);

        tmp1.add_assign(&tmp2);
        assert!(tmp1.is_zero());
    }
}

#[test]
fn test_fs_mul_assign() {
    let mut tmp = Fs(FsRepr([0xb433b01287f71744, 0x4eafb86728c4d108, 0xfdd52c14b9dfbe65, 0x2ff1f3434821118]));
    tmp.mul_assign(&Fs(FsRepr([0xdae00fc63c9fa90f, 0x5a5ed89b96ce21ce, 0x913cd26101bd6f58, 0x3f0822831697fe9])));
    assert!(tmp == Fs(FsRepr([0xb68ecb61d54d2992, 0x5ff95874defce6a6, 0x3590eb053894657d, 0x53823a118515933])));

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000000 {
        // Ensure that (a * b) * c = a * (b * c)
        let a = Fs::rand(&mut rng);
        let b = Fs::rand(&mut rng);
        let c = Fs::rand(&mut rng);

        let mut tmp1 = a;
        tmp1.mul_assign(&b);
        tmp1.mul_assign(&c);

        let mut tmp2 = b;
        tmp2.mul_assign(&c);
        tmp2.mul_assign(&a);

        assert_eq!(tmp1, tmp2);
    }

    for _ in 0..1000000 {
        // Ensure that r * (a + b + c) = r*a + r*b + r*c

        let r = Fs::rand(&mut rng);
        let mut a = Fs::rand(&mut rng);
        let mut b = Fs::rand(&mut rng);
        let mut c = Fs::rand(&mut rng);

        let mut tmp1 = a;
        tmp1.add_assign(&b);
        tmp1.add_assign(&c);
        tmp1.mul_assign(&r);

        a.mul_assign(&r);
        b.mul_assign(&r);
        c.mul_assign(&r);

        a.add_assign(&b);
        a.add_assign(&c);

        assert_eq!(tmp1, a);
    }
}

#[test]
fn test_fr_squaring() {
    let mut a = Fs(FsRepr([0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff, 0xe7db4ea6533afa8]));
    assert!(a.is_valid());
    a.square();
    assert_eq!(a, Fs::from_repr(FsRepr([0x12c7f55cbc52fbaa, 0xdedc98a0b5e6ce9e, 0xad2892726a5396a, 0x9fe82af8fee77b3])).unwrap());

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000000 {
        // Ensure that (a * a) = a^2
        let a = Fs::rand(&mut rng);

        let mut tmp = a;
        tmp.square();

        let mut tmp2 = a;
        tmp2.mul_assign(&a);

        assert_eq!(tmp, tmp2);
    }
}

#[test]
fn test_fs_inverse() {
    assert!(Fs::zero().inverse().is_none());

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let one = Fs::one();

    for _ in 0..1000 {
        // Ensure that a * a^-1 = 1
        let mut a = Fs::rand(&mut rng);
        let ainv = a.inverse().unwrap();
        a.mul_assign(&ainv);
        assert_eq!(a, one);
    }
}

#[test]
fn test_fs_double() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        // Ensure doubling a is equivalent to adding a to itself.
        let mut a = Fs::rand(&mut rng);
        let mut b = a;
        b.add_assign(&a);
        a.double();
        assert_eq!(a, b);
    }
}

#[test]
fn test_fs_negate() {
    {
        let mut a = Fs::zero();
        a.negate();

        assert!(a.is_zero());
    }

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        // Ensure (a - (-a)) = 0.
        let mut a = Fs::rand(&mut rng);
        let mut b = a;
        b.negate();
        a.add_assign(&b);

        assert!(a.is_zero());
    }
}

#[test]
fn test_fs_pow() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for i in 0..1000 {
        // Exponentiate by various small numbers and ensure it consists with repeated
        // multiplication.
        let a = Fs::rand(&mut rng);
        let target = a.pow(&[i]);
        let mut c = Fs::one();
        for _ in 0..i {
            c.mul_assign(&a);
        }
        assert_eq!(c, target);
    }

    for _ in 0..1000 {
        // Exponentiating by the modulus should have no effect in a prime field.
        let a = Fs::rand(&mut rng);

        assert_eq!(a, a.pow(Fs::char()));
    }
}

#[test]
fn test_fs_sqrt() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    assert_eq!(Fs::zero().sqrt().unwrap(), Fs::zero());

    for _ in 0..1000 {
        // Ensure sqrt(a^2) = a or -a
        let a = Fs::rand(&mut rng);
        let mut nega = a;
        nega.negate();
        let mut b = a;
        b.square();

        let b = b.sqrt().unwrap();

        assert!(a == b || nega == b);
    }

    for _ in 0..1000 {
        // Ensure sqrt(a)^2 = a for random a
        let a = Fs::rand(&mut rng);

        if let Some(mut tmp) = a.sqrt() {
            tmp.square();

            assert_eq!(a, tmp);
        }
    }
}

#[test]
fn test_fs_from_into_repr() {
    // r + 1 should not be in the field
    assert!(Fs::from_repr(FsRepr([0xd0970e5ed6f72cb8, 0xa6682093ccc81082, 0x6673b0101343b00, 0xe7db4ea6533afa9])).is_err());

    // r should not be in the field
    assert!(Fs::from_repr(Fs::char()).is_err());

    // Multiply some arbitrary representations to see if the result is as expected.
    let a = FsRepr([0x5f2d0c05d0337b71, 0xa1df2b0f8a20479, 0xad73785e71bb863, 0x504a00480c9acec]);
    let mut a_fs = Fs::from_repr(a).unwrap();
    let b = FsRepr([0x66356ff51e477562, 0x60a92ab55cf7603, 0x8e4273c7364dd192, 0x36df8844a344dc5]);
    let b_fs = Fs::from_repr(b).unwrap();
    let c = FsRepr([0x7eef61708f4f2868, 0x747a7e6cf52946fb, 0x83dd75d7c9120017, 0x762f5177f0f3df7]);
    a_fs.mul_assign(&b_fs);
    assert_eq!(a_fs.into_repr(), c);

    // Zero should be in the field.
    assert!(Fs::from_repr(FsRepr::from(0)).unwrap().is_zero());

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        // Try to turn Fs elements into representations and back again, and compare.
        let a = Fs::rand(&mut rng);
        let a_repr = a.into_repr();
        let b_repr = FsRepr::from(a);
        assert_eq!(a_repr, b_repr);
        let a_again = Fs::from_repr(a_repr).unwrap();

        assert_eq!(a, a_again);
    }
}

#[test]
fn test_fs_repr_display() {
    assert_eq!(
        format!("{}", FsRepr([0xa296db59787359df, 0x8d3e33077430d318, 0xd1abf5c606102eb7, 0xcbc33ee28108f0])),
        "0x00cbc33ee28108f0d1abf5c606102eb78d3e33077430d318a296db59787359df".to_string()
    );
    assert_eq!(
        format!("{}", FsRepr([0x14cb03535054a620, 0x312aa2bf2d1dff52, 0x970fe98746ab9361, 0xc1e18acf82711e6])),
        "0x0c1e18acf82711e6970fe98746ab9361312aa2bf2d1dff5214cb03535054a620".to_string()
    );
    assert_eq!(
        format!("{}", FsRepr([0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff])),
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string()
    );
    assert_eq!(
        format!("{}", FsRepr([0, 0, 0, 0])),
        "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
    );
}

#[test]
fn test_fs_display() {
    assert_eq!(
        format!("{}", Fs::from_repr(FsRepr([0x5528efb9998a01a3, 0x5bd2add5cb357089, 0xc061fa6adb491f98, 0x70db9d143db03d9])).unwrap()),
        "Fs(0x070db9d143db03d9c061fa6adb491f985bd2add5cb3570895528efb9998a01a3)".to_string()
    );
    assert_eq!(
        format!("{}", Fs::from_repr(FsRepr([0xd674745e2717999e, 0xbeb1f52d3e96f338, 0x9c7ae147549482b9, 0x999706024530d22])).unwrap()),
        "Fs(0x0999706024530d229c7ae147549482b9beb1f52d3e96f338d674745e2717999e)".to_string()
    );
}

#[test]
fn test_fs_num_bits() {
    assert_eq!(Fs::NUM_BITS, 252);
    assert_eq!(Fs::CAPACITY, 251);
}

#[test]
fn test_fs_root_of_unity() {
    assert_eq!(Fs::S, 1);
    assert_eq!(Fs::multiplicative_generator(), Fs::from_repr(FsRepr::from(6)).unwrap());
    assert_eq!(
        Fs::multiplicative_generator().pow([0x684b872f6b7b965b, 0x53341049e6640841, 0x83339d80809a1d80, 0x73eda753299d7d4]),
        Fs::root_of_unity()
    );
    assert_eq!(
        Fs::root_of_unity().pow([1 << Fs::S]),
        Fs::one()
    );
    assert!(Fs::multiplicative_generator().sqrt().is_none());
}
