use super::fq::{FROBENIUS_COEFF_FQ2_C1, Fq, NEGATIVE_ONE};
use ff::{Field, SqrtField};
use rand::{Rand, Rng};

use std::cmp::Ordering;

/// An element of Fq2, represented by c0 + c1 * u.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Fq2 {
    pub c0: Fq,
    pub c1: Fq,
}

impl ::std::fmt::Display for Fq2 {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "Fq2({} + {} * u)", self.c0, self.c1)
    }
}

/// `Fq2` elements are ordered lexicographically.
impl Ord for Fq2 {
    #[inline(always)]
    fn cmp(&self, other: &Fq2) -> Ordering {
        match self.c1.cmp(&other.c1) {
            Ordering::Greater => Ordering::Greater,
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.c0.cmp(&other.c0),
        }
    }
}

impl PartialOrd for Fq2 {
    #[inline(always)]
    fn partial_cmp(&self, other: &Fq2) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Fq2 {
    /// Multiply this element by the cubic and quadratic nonresidue 1 + u.
    pub fn mul_by_nonresidue(&mut self) {
        let t0 = self.c0;
        self.c0.sub_assign(&self.c1);
        self.c1.add_assign(&t0);
    }

    /// Norm of Fq2 as extension field in i over Fq
    pub fn norm(&self) -> Fq {
        let mut t0 = self.c0;
        let mut t1 = self.c1;
        t0.square();
        t1.square();
        t1.add_assign(&t0);

        t1
    }
}

impl Rand for Fq2 {
    fn rand<R: Rng>(rng: &mut R) -> Self {
        Fq2 {
            c0: rng.gen(),
            c1: rng.gen(),
        }
    }
}

impl Field for Fq2 {
    fn zero() -> Self {
        Fq2 {
            c0: Fq::zero(),
            c1: Fq::zero(),
        }
    }

    fn one() -> Self {
        Fq2 {
            c0: Fq::one(),
            c1: Fq::zero(),
        }
    }

    fn is_zero(&self) -> bool {
        self.c0.is_zero() && self.c1.is_zero()
    }

    fn square(&mut self) {
        let mut ab = self.c0;
        ab.mul_assign(&self.c1);
        let mut c0c1 = self.c0;
        c0c1.add_assign(&self.c1);
        let mut c0 = self.c1;
        c0.negate();
        c0.add_assign(&self.c0);
        c0.mul_assign(&c0c1);
        c0.sub_assign(&ab);
        self.c1 = ab;
        self.c1.add_assign(&ab);
        c0.add_assign(&ab);
        self.c0 = c0;
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
        self.c0 = aa;
        self.c0.sub_assign(&bb);
    }

    fn inverse(&self) -> Option<Self> {
        let mut t1 = self.c1;
        t1.square();
        let mut t0 = self.c0;
        t0.square();
        t0.add_assign(&t1);
        t0.inverse().map(|t| {
            let mut tmp = Fq2 {
                c0: self.c0,
                c1: self.c1,
            };
            tmp.c0.mul_assign(&t);
            tmp.c1.mul_assign(&t);
            tmp.c1.negate();

            tmp
        })
    }

    fn frobenius_map(&mut self, power: usize) {
        self.c1.mul_assign(&FROBENIUS_COEFF_FQ2_C1[power % 2]);
    }
}

impl SqrtField for Fq2 {
    fn legendre(&self) -> ::ff::LegendreSymbol {
        self.norm().legendre()
    }

    fn sqrt(&self) -> Option<Self> {
        // Algorithm 9, https://eprint.iacr.org/2012/685.pdf

        if self.is_zero() {
            Some(Self::zero())
        } else {
            // a1 = self^((q - 3) / 4)
            let mut a1 = self.pow([
                0xee7fbfffffffeaaa,
                0x7aaffffac54ffff,
                0xd9cc34a83dac3d89,
                0xd91dd2e13ce144af,
                0x92c6e9ed90d2eb35,
                0x680447a8e5ff9a6,
            ]);
            let mut alpha = a1;
            alpha.square();
            alpha.mul_assign(self);
            let mut a0 = alpha;
            a0.frobenius_map(1);
            a0.mul_assign(&alpha);

            let neg1 = Fq2 {
                c0: NEGATIVE_ONE,
                c1: Fq::zero(),
            };

            if a0 == neg1 {
                None
            } else {
                a1.mul_assign(self);

                if alpha == neg1 {
                    a1.mul_assign(&Fq2 {
                        c0: Fq::zero(),
                        c1: Fq::one(),
                    });
                } else {
                    alpha.add_assign(&Fq2::one());
                    // alpha = alpha^((q - 1) / 2)
                    alpha = alpha.pow([
                        0xdcff7fffffffd555,
                        0xf55ffff58a9ffff,
                        0xb39869507b587b12,
                        0xb23ba5c279c2895f,
                        0x258dd3db21a5d66b,
                        0xd0088f51cbff34d,
                    ]);
                    a1.mul_assign(&alpha);
                }

                Some(a1)
            }
        }
    }
}

#[test]
fn test_fq2_ordering() {
    let mut a = Fq2 {
        c0: Fq::zero(),
        c1: Fq::zero(),
    };

    let mut b = a.clone();

    assert!(a.cmp(&b) == Ordering::Equal);
    b.c0.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Less);
    a.c0.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Equal);
    b.c1.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Less);
    a.c0.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Less);
    a.c1.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Greater);
    b.c0.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Equal);
}

#[test]
fn test_fq2_basics() {
    assert_eq!(
        Fq2 {
            c0: Fq::zero(),
            c1: Fq::zero(),
        },
        Fq2::zero()
    );
    assert_eq!(
        Fq2 {
            c0: Fq::one(),
            c1: Fq::zero(),
        },
        Fq2::one()
    );
    assert!(Fq2::zero().is_zero());
    assert!(!Fq2::one().is_zero());
    assert!(
        !Fq2 {
            c0: Fq::zero(),
            c1: Fq::one(),
        }.is_zero()
    );
}

#[test]
fn test_fq2_squaring() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    let mut a = Fq2 {
        c0: Fq::one(),
        c1: Fq::one(),
    }; // u + 1
    a.square();
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::zero(),
            c1: Fq::from_repr(FqRepr::from(2)).unwrap(),
        }
    ); // 2u

    let mut a = Fq2 {
        c0: Fq::zero(),
        c1: Fq::one(),
    }; // u
    a.square();
    assert_eq!(a, {
        let mut neg1 = Fq::one();
        neg1.negate();
        Fq2 {
            c0: neg1,
            c1: Fq::zero(),
        }
    }); // -1

    let mut a = Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x9c2c6309bbf8b598,
            0x4eef5c946536f602,
            0x90e34aab6fb6a6bd,
            0xf7f295a94e58ae7c,
            0x41b76dcc1c3fbe5e,
            0x7080c5fa1d8e042,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x38f473b3c870a4ab,
            0x6ad3291177c8c7e5,
            0xdac5a4c911a4353e,
            0xbfb99020604137a0,
            0xfc58a7b7be815407,
            0x10d1615e75250a21,
        ])).unwrap(),
    };
    a.square();
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0xf262c28c538bcf68,
                0xb9f2a66eae1073ba,
                0xdc46ab8fad67ae0,
                0xcb674157618da176,
                0x4cf17b5893c3d327,
                0x7eac81369c43361
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0xc1579cf58e980cf8,
                0xa23eb7e12dd54d98,
                0xe75138bce4cec7aa,
                0x38d0d7275a9689e1,
                0x739c983042779a65,
                0x1542a61c8a8db994
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_mul() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    let mut a = Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x85c9f989e1461f03,
            0xa2e33c333449a1d6,
            0x41e461154a7354a3,
            0x9ee53e7e84d7532e,
            0x1c202d8ed97afb45,
            0x51d3f9253e2516f,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0xa7348a8b511aedcf,
            0x143c215d8176b319,
            0x4cc48081c09b8903,
            0x9533e4a9a5158be,
            0x7a5e1ecb676d65f9,
            0x180c3ee46656b008,
        ])).unwrap(),
    };
    a.mul_assign(&Fq2 {
        c0: Fq::from_repr(FqRepr([
            0xe21f9169805f537e,
            0xfc87e62e179c285d,
            0x27ece175be07a531,
            0xcd460f9f0c23e430,
            0x6c9110292bfa409,
            0x2c93a72eb8af83e,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x4b1c3f936d8992d4,
            0x1d2a72916dba4c8a,
            0x8871c508658d1e5f,
            0x57a06d3135a752ae,
            0x634cd3c6c565096d,
            0x19e17334d4e93558,
        ])).unwrap(),
    });
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x95b5127e6360c7e4,
                0xde29c31a19a6937e,
                0xf61a96dacf5a39bc,
                0x5511fe4d84ee5f78,
                0x5310a202d92f9963,
                0x1751afbe166e5399
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0x84af0e1bd630117a,
                0x6c63cd4da2c2aa7,
                0x5ba6e5430e883d40,
                0xc975106579c275ee,
                0x33a9ac82ce4c5083,
                0x1ef1a36c201589d
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_inverse() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    assert!(Fq2::zero().inverse().is_none());

    let a = Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x85c9f989e1461f03,
            0xa2e33c333449a1d6,
            0x41e461154a7354a3,
            0x9ee53e7e84d7532e,
            0x1c202d8ed97afb45,
            0x51d3f9253e2516f,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0xa7348a8b511aedcf,
            0x143c215d8176b319,
            0x4cc48081c09b8903,
            0x9533e4a9a5158be,
            0x7a5e1ecb676d65f9,
            0x180c3ee46656b008,
        ])).unwrap(),
    };
    let a = a.inverse().unwrap();
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x70300f9bcb9e594,
                0xe5ecda5fdafddbb2,
                0x64bef617d2915a8f,
                0xdfba703293941c30,
                0xa6c3d8f9586f2636,
                0x1351ef01941b70c4
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0x8c39fd76a8312cb4,
                0x15d7b6b95defbff0,
                0x947143f89faedee9,
                0xcbf651a0f367afb2,
                0xdf4e54f0d3ef15a6,
                0x103bdf241afb0019
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_addition() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    let mut a = Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x2d0078036923ffc7,
            0x11e59ea221a3b6d2,
            0x8b1a52e0a90f59ed,
            0xb966ce3bc2108b13,
            0xccc649c4b9532bf3,
            0xf8d295b2ded9dc,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x977df6efcdaee0db,
            0x946ae52d684fa7ed,
            0xbe203411c66fb3a5,
            0xb3f8afc0ee248cad,
            0x4e464dea5bcfd41e,
            0x12d1137b8a6a837,
        ])).unwrap(),
    };
    a.add_assign(&Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x619a02d78dc70ef2,
            0xb93adfc9119e33e8,
            0x4bf0b99a9f0dca12,
            0x3b88899a42a6318f,
            0x986a4a62fa82a49d,
            0x13ce433fa26027f5,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x66323bf80b58b9b9,
            0xa1379b6facf6e596,
            0x402aef1fb797e32f,
            0x2236f55246d0d44d,
            0x4c8c1800eb104566,
            0x11d6e20e986c2085,
        ])).unwrap(),
    });
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x8e9a7adaf6eb0eb9,
                0xcb207e6b3341eaba,
                0xd70b0c7b481d23ff,
                0xf4ef57d604b6bca2,
                0x65309427b3d5d090,
                0x14c715d5553f01d2
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0xfdb032e7d9079a94,
                0x35a2809d15468d83,
                0xfe4b23317e0796d5,
                0xd62fa51334f560fa,
                0x9ad265eb46e01984,
                0x1303f3465112c8bc
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_subtraction() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    let mut a = Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x2d0078036923ffc7,
            0x11e59ea221a3b6d2,
            0x8b1a52e0a90f59ed,
            0xb966ce3bc2108b13,
            0xccc649c4b9532bf3,
            0xf8d295b2ded9dc,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x977df6efcdaee0db,
            0x946ae52d684fa7ed,
            0xbe203411c66fb3a5,
            0xb3f8afc0ee248cad,
            0x4e464dea5bcfd41e,
            0x12d1137b8a6a837,
        ])).unwrap(),
    };
    a.sub_assign(&Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x619a02d78dc70ef2,
            0xb93adfc9119e33e8,
            0x4bf0b99a9f0dca12,
            0x3b88899a42a6318f,
            0x986a4a62fa82a49d,
            0x13ce433fa26027f5,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x66323bf80b58b9b9,
            0xa1379b6facf6e596,
            0x402aef1fb797e32f,
            0x2236f55246d0d44d,
            0x4c8c1800eb104566,
            0x11d6e20e986c2085,
        ])).unwrap(),
    });
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x8565752bdb5c9b80,
                0x7756bed7c15982e9,
                0xa65a6be700b285fe,
                0xe255902672ef6c43,
                0x7f77a718021c342d,
                0x72ba14049fe9881
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0xeb4abaf7c255d1cd,
                0x11df49bc6cacc256,
                0xe52617930588c69a,
                0xf63905f39ad8cb1f,
                0x4cd5dd9fb40b3b8f,
                0x957411359ba6e4c
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_negation() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    let mut a = Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x2d0078036923ffc7,
            0x11e59ea221a3b6d2,
            0x8b1a52e0a90f59ed,
            0xb966ce3bc2108b13,
            0xccc649c4b9532bf3,
            0xf8d295b2ded9dc,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x977df6efcdaee0db,
            0x946ae52d684fa7ed,
            0xbe203411c66fb3a5,
            0xb3f8afc0ee248cad,
            0x4e464dea5bcfd41e,
            0x12d1137b8a6a837,
        ])).unwrap(),
    };
    a.negate();
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x8cfe87fc96dbaae4,
                0xcc6615c8fb0492d,
                0xdc167fc04da19c37,
                0xab107d49317487ab,
                0x7e555df189f880e3,
                0x19083f5486a10cbd
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0x228109103250c9d0,
                0x8a411ad149045812,
                0xa9109e8f3041427e,
                0xb07e9bc405608611,
                0xfcd559cbe77bd8b8,
                0x18d400b280d93e62
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_doubling() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    let mut a = Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x2d0078036923ffc7,
            0x11e59ea221a3b6d2,
            0x8b1a52e0a90f59ed,
            0xb966ce3bc2108b13,
            0xccc649c4b9532bf3,
            0xf8d295b2ded9dc,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x977df6efcdaee0db,
            0x946ae52d684fa7ed,
            0xbe203411c66fb3a5,
            0xb3f8afc0ee248cad,
            0x4e464dea5bcfd41e,
            0x12d1137b8a6a837,
        ])).unwrap(),
    };
    a.double();
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x5a00f006d247ff8e,
                0x23cb3d4443476da4,
                0x1634a5c1521eb3da,
                0x72cd9c7784211627,
                0x998c938972a657e7,
                0x1f1a52b65bdb3b9
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0x2efbeddf9b5dc1b6,
                0x28d5ca5ad09f4fdb,
                0x7c4068238cdf674b,
                0x67f15f81dc49195b,
                0x9c8c9bd4b79fa83d,
                0x25a226f714d506e
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_frobenius_map() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    let mut a = Fq2 {
        c0: Fq::from_repr(FqRepr([
            0x2d0078036923ffc7,
            0x11e59ea221a3b6d2,
            0x8b1a52e0a90f59ed,
            0xb966ce3bc2108b13,
            0xccc649c4b9532bf3,
            0xf8d295b2ded9dc,
        ])).unwrap(),
        c1: Fq::from_repr(FqRepr([
            0x977df6efcdaee0db,
            0x946ae52d684fa7ed,
            0xbe203411c66fb3a5,
            0xb3f8afc0ee248cad,
            0x4e464dea5bcfd41e,
            0x12d1137b8a6a837,
        ])).unwrap(),
    };
    a.frobenius_map(0);
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x2d0078036923ffc7,
                0x11e59ea221a3b6d2,
                0x8b1a52e0a90f59ed,
                0xb966ce3bc2108b13,
                0xccc649c4b9532bf3,
                0xf8d295b2ded9dc
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0x977df6efcdaee0db,
                0x946ae52d684fa7ed,
                0xbe203411c66fb3a5,
                0xb3f8afc0ee248cad,
                0x4e464dea5bcfd41e,
                0x12d1137b8a6a837
            ])).unwrap(),
        }
    );
    a.frobenius_map(1);
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x2d0078036923ffc7,
                0x11e59ea221a3b6d2,
                0x8b1a52e0a90f59ed,
                0xb966ce3bc2108b13,
                0xccc649c4b9532bf3,
                0xf8d295b2ded9dc
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0x228109103250c9d0,
                0x8a411ad149045812,
                0xa9109e8f3041427e,
                0xb07e9bc405608611,
                0xfcd559cbe77bd8b8,
                0x18d400b280d93e62
            ])).unwrap(),
        }
    );
    a.frobenius_map(1);
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x2d0078036923ffc7,
                0x11e59ea221a3b6d2,
                0x8b1a52e0a90f59ed,
                0xb966ce3bc2108b13,
                0xccc649c4b9532bf3,
                0xf8d295b2ded9dc
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0x977df6efcdaee0db,
                0x946ae52d684fa7ed,
                0xbe203411c66fb3a5,
                0xb3f8afc0ee248cad,
                0x4e464dea5bcfd41e,
                0x12d1137b8a6a837
            ])).unwrap(),
        }
    );
    a.frobenius_map(2);
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x2d0078036923ffc7,
                0x11e59ea221a3b6d2,
                0x8b1a52e0a90f59ed,
                0xb966ce3bc2108b13,
                0xccc649c4b9532bf3,
                0xf8d295b2ded9dc
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0x977df6efcdaee0db,
                0x946ae52d684fa7ed,
                0xbe203411c66fb3a5,
                0xb3f8afc0ee248cad,
                0x4e464dea5bcfd41e,
                0x12d1137b8a6a837
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_sqrt() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    assert_eq!(
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x476b4c309720e227,
                0x34c2d04faffdab6,
                0xa57e6fc1bab51fd9,
                0xdb4a116b5bf74aa1,
                0x1e58b2159dfe10e2,
                0x7ca7da1f13606ac
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0xfa8de88b7516d2c3,
                0x371a75ed14f41629,
                0x4cec2dca577a3eb6,
                0x212611bca4e99121,
                0x8ee5394d77afb3d,
                0xec92336650e49d5
            ])).unwrap(),
        }.sqrt()
            .unwrap(),
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0x40b299b2704258c5,
                0x6ef7de92e8c68b63,
                0x6d2ddbe552203e82,
                0x8d7f1f723d02c1d3,
                0x881b3e01b611c070,
                0x10f6963bbad2ebc5
            ])).unwrap(),
            c1: Fq::from_repr(FqRepr([
                0xc099534fc209e752,
                0x7670594665676447,
                0x28a20faed211efe7,
                0x6b852aeaf2afcb1b,
                0xa4c93b08105d71a9,
                0x8d7cfff94216330
            ])).unwrap(),
        }
    );

    assert_eq!(
        Fq2 {
            c0: Fq::from_repr(FqRepr([
                0xb9f78429d1517a6b,
                0x1eabfffeb153ffff,
                0x6730d2a0f6b0f624,
                0x64774b84f38512bf,
                0x4b1ba7b6434bacd7,
                0x1a0111ea397fe69a
            ])).unwrap(),
            c1: Fq::zero(),
        }.sqrt()
            .unwrap(),
        Fq2 {
            c0: Fq::zero(),
            c1: Fq::from_repr(FqRepr([
                0xb9fefffffd4357a3,
                0x1eabfffeb153ffff,
                0x6730d2a0f6b0f624,
                0x64774b84f38512bf,
                0x4b1ba7b6434bacd7,
                0x1a0111ea397fe69a
            ])).unwrap(),
        }
    );
}

#[test]
fn test_fq2_legendre() {
    use ff::LegendreSymbol::*;

    assert_eq!(Zero, Fq2::zero().legendre());
    // i^2 = -1
    let mut m1 = Fq2::one();
    m1.negate();
    assert_eq!(QuadraticResidue, m1.legendre());
    m1.mul_by_nonresidue();
    assert_eq!(QuadraticNonResidue, m1.legendre());
}

#[cfg(test)]
use rand::{SeedableRng, XorShiftRng};

#[test]
fn test_fq2_mul_nonresidue() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let nqr = Fq2 {
        c0: Fq::one(),
        c1: Fq::one(),
    };

    for _ in 0..1000 {
        let mut a = Fq2::rand(&mut rng);
        let mut b = a;
        a.mul_by_nonresidue();
        b.mul_assign(&nqr);

        assert_eq!(a, b);
    }
}

#[test]
fn fq2_field_tests() {
    use ff::PrimeField;

    ::tests::field::random_field_tests::<Fq2>();
    ::tests::field::random_sqrt_tests::<Fq2>();
    ::tests::field::random_frobenius_tests::<Fq2, _>(super::fq::Fq::char(), 13);
}
