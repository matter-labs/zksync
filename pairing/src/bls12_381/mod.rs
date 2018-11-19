mod ec;
mod fq;
mod fq12;
mod fq2;
mod fq6;
mod fr;

#[cfg(test)]
mod tests;

pub use self::ec::{
    G1, G1Affine, G1Compressed, G1Prepared, G1Uncompressed, G2, G2Affine, G2Compressed, G2Prepared,
    G2Uncompressed,
};
pub use self::fq::{Fq, FqRepr};
pub use self::fq12::Fq12;
pub use self::fq2::Fq2;
pub use self::fq6::Fq6;
pub use self::fr::{Fr, FrRepr};

use super::{CurveAffine, Engine};

use ff::{BitIterator, Field, ScalarEngine};

// The BLS parameter x for BLS12-381 is -0xd201000000010000
const BLS_X: u64 = 0xd201000000010000;
const BLS_X_IS_NEGATIVE: bool = true;

#[derive(Clone, Debug)]
pub struct Bls12;

impl ScalarEngine for Bls12 {
    type Fr = Fr;
}

impl Engine for Bls12 {
    type G1 = G1;
    type G1Affine = G1Affine;
    type G2 = G2;
    type G2Affine = G2Affine;
    type Fq = Fq;
    type Fqe = Fq2;
    type Fqk = Fq12;

    fn miller_loop<'a, I>(i: I) -> Self::Fqk
    where
        I: IntoIterator<
            Item = &'a (
                &'a <Self::G1Affine as CurveAffine>::Prepared,
                &'a <Self::G2Affine as CurveAffine>::Prepared,
            ),
        >,
    {
        let mut pairs = vec![];
        for &(p, q) in i {
            if !p.is_zero() && !q.is_zero() {
                pairs.push((p, q.coeffs.iter()));
            }
        }

        // Twisting isomorphism from E to E'
        fn ell(f: &mut Fq12, coeffs: &(Fq2, Fq2, Fq2), p: &G1Affine) {
            let mut c0 = coeffs.0;
            let mut c1 = coeffs.1;

            c0.c0.mul_assign(&p.y);
            c0.c1.mul_assign(&p.y);

            c1.c0.mul_assign(&p.x);
            c1.c1.mul_assign(&p.x);

            // Sparse multiplication in Fq12
            f.mul_by_014(&coeffs.2, &c1, &c0);
        }

        let mut f = Fq12::one();

        let mut found_one = false;
        for i in BitIterator::new(&[BLS_X >> 1]) {
            if !found_one {
                found_one = i;
                continue;
            }

            for &mut (p, ref mut coeffs) in &mut pairs {
                ell(&mut f, coeffs.next().unwrap(), &p.0);
            }

            if i {
                for &mut (p, ref mut coeffs) in &mut pairs {
                    ell(&mut f, coeffs.next().unwrap(), &p.0);
                }
            }

            f.square();
        }

        for &mut (p, ref mut coeffs) in &mut pairs {
            ell(&mut f, coeffs.next().unwrap(), &p.0);
        }

        if BLS_X_IS_NEGATIVE {
            f.conjugate();
        }

        f
    }

    fn final_exponentiation(r: &Fq12) -> Option<Fq12> {
        let mut f1 = *r;
        f1.conjugate();

        match r.inverse() {
            Some(mut f2) => {
                let mut r = f1;
                r.mul_assign(&f2);
                f2 = r;
                r.frobenius_map(2);
                r.mul_assign(&f2);

                fn exp_by_x(f: &mut Fq12, x: u64) {
                    *f = f.pow(&[x]);
                    if BLS_X_IS_NEGATIVE {
                        f.conjugate();
                    }
                }

                let mut x = BLS_X;
                let mut y0 = r;
                y0.square();
                let mut y1 = y0;
                exp_by_x(&mut y1, x);
                x >>= 1;
                let mut y2 = y1;
                exp_by_x(&mut y2, x);
                x <<= 1;
                let mut y3 = r;
                y3.conjugate();
                y1.mul_assign(&y3);
                y1.conjugate();
                y1.mul_assign(&y2);
                y2 = y1;
                exp_by_x(&mut y2, x);
                y3 = y2;
                exp_by_x(&mut y3, x);
                y1.conjugate();
                y3.mul_assign(&y1);
                y1.conjugate();
                y1.frobenius_map(3);
                y2.frobenius_map(2);
                y1.mul_assign(&y2);
                y2 = y3;
                exp_by_x(&mut y2, x);
                y2.mul_assign(&y0);
                y2.mul_assign(&r);
                y1.mul_assign(&y2);
                y2 = y3;
                y2.frobenius_map(1);
                y1.mul_assign(&y2);

                Some(y1)
            }
            None => None,
        }
    }
}

impl G2Prepared {
    pub fn is_zero(&self) -> bool {
        self.infinity
    }

    pub fn from_affine(q: G2Affine) -> Self {
        if q.is_zero() {
            return G2Prepared {
                coeffs: vec![],
                infinity: true,
            };
        }

        fn doubling_step(r: &mut G2) -> (Fq2, Fq2, Fq2) {
            // Adaptation of Algorithm 26, https://eprint.iacr.org/2010/354.pdf
            let mut tmp0 = r.x;
            tmp0.square();

            let mut tmp1 = r.y;
            tmp1.square();

            let mut tmp2 = tmp1;
            tmp2.square();

            let mut tmp3 = tmp1;
            tmp3.add_assign(&r.x);
            tmp3.square();
            tmp3.sub_assign(&tmp0);
            tmp3.sub_assign(&tmp2);
            tmp3.double();

            let mut tmp4 = tmp0;
            tmp4.double();
            tmp4.add_assign(&tmp0);

            let mut tmp6 = r.x;
            tmp6.add_assign(&tmp4);

            let mut tmp5 = tmp4;
            tmp5.square();

            let mut zsquared = r.z;
            zsquared.square();

            r.x = tmp5;
            r.x.sub_assign(&tmp3);
            r.x.sub_assign(&tmp3);

            r.z.add_assign(&r.y);
            r.z.square();
            r.z.sub_assign(&tmp1);
            r.z.sub_assign(&zsquared);

            r.y = tmp3;
            r.y.sub_assign(&r.x);
            r.y.mul_assign(&tmp4);

            tmp2.double();
            tmp2.double();
            tmp2.double();

            r.y.sub_assign(&tmp2);

            tmp3 = tmp4;
            tmp3.mul_assign(&zsquared);
            tmp3.double();
            tmp3.negate();

            tmp6.square();
            tmp6.sub_assign(&tmp0);
            tmp6.sub_assign(&tmp5);

            tmp1.double();
            tmp1.double();

            tmp6.sub_assign(&tmp1);

            tmp0 = r.z;
            tmp0.mul_assign(&zsquared);
            tmp0.double();

            (tmp0, tmp3, tmp6)
        }

        fn addition_step(r: &mut G2, q: &G2Affine) -> (Fq2, Fq2, Fq2) {
            // Adaptation of Algorithm 27, https://eprint.iacr.org/2010/354.pdf
            let mut zsquared = r.z;
            zsquared.square();

            let mut ysquared = q.y;
            ysquared.square();

            let mut t0 = zsquared;
            t0.mul_assign(&q.x);

            let mut t1 = q.y;
            t1.add_assign(&r.z);
            t1.square();
            t1.sub_assign(&ysquared);
            t1.sub_assign(&zsquared);
            t1.mul_assign(&zsquared);

            let mut t2 = t0;
            t2.sub_assign(&r.x);

            let mut t3 = t2;
            t3.square();

            let mut t4 = t3;
            t4.double();
            t4.double();

            let mut t5 = t4;
            t5.mul_assign(&t2);

            let mut t6 = t1;
            t6.sub_assign(&r.y);
            t6.sub_assign(&r.y);

            let mut t9 = t6;
            t9.mul_assign(&q.x);

            let mut t7 = t4;
            t7.mul_assign(&r.x);

            r.x = t6;
            r.x.square();
            r.x.sub_assign(&t5);
            r.x.sub_assign(&t7);
            r.x.sub_assign(&t7);

            r.z.add_assign(&t2);
            r.z.square();
            r.z.sub_assign(&zsquared);
            r.z.sub_assign(&t3);

            let mut t10 = q.y;
            t10.add_assign(&r.z);

            let mut t8 = t7;
            t8.sub_assign(&r.x);
            t8.mul_assign(&t6);

            t0 = r.y;
            t0.mul_assign(&t5);
            t0.double();

            r.y = t8;
            r.y.sub_assign(&t0);

            t10.square();
            t10.sub_assign(&ysquared);

            let mut ztsquared = r.z;
            ztsquared.square();

            t10.sub_assign(&ztsquared);

            t9.double();
            t9.sub_assign(&t10);

            t10 = r.z;
            t10.double();

            t6.negate();

            t1 = t6;
            t1.double();

            (t10, t1, t9)
        }

        let mut coeffs = vec![];
        let mut r: G2 = q.into();

        let mut found_one = false;
        for i in BitIterator::new([BLS_X >> 1]) {
            if !found_one {
                found_one = i;
                continue;
            }

            coeffs.push(doubling_step(&mut r));

            if i {
                coeffs.push(addition_step(&mut r, &q));
            }
        }

        coeffs.push(doubling_step(&mut r));

        G2Prepared {
            coeffs,
            infinity: false,
        }
    }
}

#[test]
fn bls12_engine_tests() {
    ::tests::engine::engine_tests::<Bls12>();
}
