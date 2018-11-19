use super::fq::FROBENIUS_COEFF_FQ12_C1;
use super::fq2::Fq2;
use super::fq6::Fq6;
use ff::Field;
use rand::{Rand, Rng};

/// An element of Fq12, represented by c0 + c1 * w.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Fq12 {
    pub c0: Fq6,
    pub c1: Fq6,
}

impl ::std::fmt::Display for Fq12 {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "Fq12({} + {} * w)", self.c0, self.c1)
    }
}

impl Rand for Fq12 {
    fn rand<R: Rng>(rng: &mut R) -> Self {
        Fq12 {
            c0: rng.gen(),
            c1: rng.gen(),
        }
    }
}

// BN256 and BLS12 implementations should be the same
// Defined over w^2 - v = 0

impl Fq12 {
    pub fn conjugate(&mut self) {
        self.c1.negate();
    }

    pub fn mul_by_014(&mut self, c0: &Fq2, c1: &Fq2, c4: &Fq2) {
        let mut aa = self.c0;
        aa.mul_by_01(c0, c1);
        let mut bb = self.c1;
        bb.mul_by_1(c4);
        let mut o = *c1;
        o.add_assign(c4);
        self.c1.add_assign(&self.c0);
        self.c1.mul_by_01(c0, &o);
        self.c1.sub_assign(&aa);
        self.c1.sub_assign(&bb);
        self.c0 = bb;
        self.c0.mul_by_nonresidue();
        self.c0.add_assign(&aa);
    }
    // TODO make it hand optimized
    // // multiply by (c0, c1, c2) + (c3, c4, c5)*w where only c0, c3 and c4 are non-zero
    pub fn mul_by_034(&mut self, c0: &Fq2, c3: &Fq2, c4: &Fq2) {
        self.mul_assign(&Fq12 {
                c0: Fq6 {
                    c0: *c0,
                    c1: Fq2::zero(),
                    c2: Fq2::zero(),
                },
                c1: Fq6 {
                    c0: *c3,
                    c1: *c4,
                    c2: Fq2::zero(),
                },
        });
    }
}

impl Field for Fq12 {
    fn zero() -> Self {
        Fq12 {
            c0: Fq6::zero(),
            c1: Fq6::zero(),
        }
    }

    fn one() -> Self {
        Fq12 {
            c0: Fq6::one(),
            c1: Fq6::zero(),
        }
    }

    fn is_zero(&self) -> bool {
        self.c0.is_zero() && self.c1.is_zero()
    }

    fn double(&mut self) {
        self.c0.double();
        self.c1.double();
    }

    fn negate(&mut self) {
        self.c0.negate();
        self.c1.negate();
    }

    fn add_assign(&mut self, other: &Self) {
        self.c0.add_assign(&other.c0);
        self.c1.add_assign(&other.c1);
    }

    fn sub_assign(&mut self, other: &Self) {
        self.c0.sub_assign(&other.c0);
        self.c1.sub_assign(&other.c1);
    }

    fn frobenius_map(&mut self, power: usize) {
        self.c0.frobenius_map(power);
        self.c1.frobenius_map(power);

        self.c1.c0.mul_assign(&FROBENIUS_COEFF_FQ12_C1[power % 12]);
        self.c1.c1.mul_assign(&FROBENIUS_COEFF_FQ12_C1[power % 12]);
        self.c1.c2.mul_assign(&FROBENIUS_COEFF_FQ12_C1[power % 12]);
    }

    fn square(&mut self) {
        let mut ab = self.c0;
        ab.mul_assign(&self.c1);
        let mut c0c1 = self.c0;
        c0c1.add_assign(&self.c1);
        let mut c0 = self.c1;
        c0.mul_by_nonresidue();
        c0.add_assign(&self.c0);
        c0.mul_assign(&c0c1);
        c0.sub_assign(&ab);
        self.c1 = ab;
        self.c1.add_assign(&ab);
        ab.mul_by_nonresidue();
        c0.sub_assign(&ab);
        self.c0 = c0;
    }

    fn mul_assign(&mut self, other: &Self) {
        let mut aa = self.c0;
        aa.mul_assign(&other.c0);
        let mut bb = self.c1;
        bb.mul_assign(&other.c1);
        let mut o = other.c0;
        o.add_assign(&other.c1);
        self.c1.add_assign(&self.c0);
        self.c1.mul_assign(&o);
        self.c1.sub_assign(&aa);
        self.c1.sub_assign(&bb);
        self.c0 = bb;
        self.c0.mul_by_nonresidue();
        self.c0.add_assign(&aa);
    }

    fn inverse(&self) -> Option<Self> {
        let mut c0s = self.c0;
        c0s.square();
        let mut c1s = self.c1;
        c1s.square();
        c1s.mul_by_nonresidue();
        c0s.sub_assign(&c1s);

        c0s.inverse().map(|t| {
            let mut tmp = Fq12 { c0: t, c1: t };
            tmp.c0.mul_assign(&self.c0);
            tmp.c1.mul_assign(&self.c1);
            tmp.c1.negate();

            tmp
        })
    }
}

#[cfg(test)]
use rand::{SeedableRng, XorShiftRng};

#[test]
fn test_fq12_mul_by_014() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let c0 = Fq2::rand(&mut rng);
        let c1 = Fq2::rand(&mut rng);
        let c5 = Fq2::rand(&mut rng);
        let mut a = Fq12::rand(&mut rng);
        let mut b = a;

        a.mul_by_014(&c0, &c1, &c5);
        b.mul_assign(&Fq12 {
            c0: Fq6 {
                c0: c0,
                c1: c1,
                c2: Fq2::zero(),
            },
            c1: Fq6 {
                c0: Fq2::zero(),
                c1: c5,
                c2: Fq2::zero(),
            },
        });

        assert_eq!(a, b);
    }
}

#[test]
fn test_squaring() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let mut a = Fq12::rand(&mut rng);
        let mut b = a;
        b.mul_assign(&a);
        a.square();
        assert_eq!(a, b);
    }
}

#[test]
fn fq12_field_tests() {
    use ff::PrimeField;

    ::tests::field::random_field_tests::<Fq12>();
    ::tests::field::random_frobenius_tests::<Fq12, _>(super::fq::Fq::char(), 13);
}
