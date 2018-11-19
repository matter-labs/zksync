mod ec;
mod fq;
mod fq12;
mod fq2;
mod fq6;
mod fr;

// #[cfg(test)]
// mod tests;

pub use self::ec::{
    G1, G1Affine, G1Compressed, G1Prepared, G1Uncompressed,
    G2, G2Affine, G2Compressed, G2Prepared, G2Uncompressed,
};
pub use self::fq::{Fq, FqRepr, FROBENIUS_COEFF_FQ6_C1, XI_TO_Q_MINUS_1_OVER_2};
pub use self::fq12::Fq12;
pub use self::fq2::Fq2;
pub use self::fq6::Fq6;
pub use self::fr::{Fr, FrRepr};

use super::{CurveAffine, Engine};

use ff::{Field, ScalarEngine};

#[derive(Clone, Debug)]
pub struct Bn256;

// U value that originates this particular curve
pub const BN_U: u64 = 4965661367192848881;

// // 6U+2 for in NAF form
pub const SIX_U_PLUS_2_NAF : [i8; 65] = [0, 0, 0, 1, 0, 1, 0, -1, 0, 0, 1, -1, 0, 0, 1, 0,
	0, 1, 1, 0, -1, 0, 0, 1, 0, -1, 0, 0, 0, 0, 1, 1,
	1, 0, 0, -1, 0, 0, 1, 0, 0, 0, 0, 0, -1, 0, 0, 1,
	1, 0, 0, -1, 0, 0, 0, 1, 1, 0, -1, 0, 0, 1, 0, 1, 1];


impl ScalarEngine for Bn256 {
    type Fr = Fr;
}

impl Engine for Bn256 {
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

        // Final steps of the line function on prepared coefficients
        fn ell(f: &mut Fq12, coeffs: &(Fq2, Fq2, Fq2), p: &G1Affine) {
            let mut c0 = coeffs.0;
            let mut c1 = coeffs.1;

            c0.c0.mul_assign(&p.y);
            c0.c1.mul_assign(&p.y);

            c1.c0.mul_assign(&p.x);
            c1.c1.mul_assign(&p.x);

            // Sparse multiplication in Fq12

            f.mul_by_034(&c0, &c1, &coeffs.2);
        }

        let mut f = Fq12::one();
        
        for i in (1..SIX_U_PLUS_2_NAF.len()).rev() {
            if i != SIX_U_PLUS_2_NAF.len() - 1 {
                f.square();
            }
            for &mut (p, ref mut coeffs) in &mut pairs {
                ell(&mut f, coeffs.next().unwrap(), &p.0);
            }
            let x = SIX_U_PLUS_2_NAF[i-1];
            match x {
                1 => {
                        for &mut (p, ref mut coeffs) in &mut pairs {
                            ell(&mut f, coeffs.next().unwrap(), &p.0);
                        }
                    }
                -1 => {
                        for &mut (p, ref mut coeffs) in &mut pairs {
                            ell(&mut f, coeffs.next().unwrap(), &p.0);
                        }
                    }
                _ => {
                    continue
                }
            }
        }

        // two additional steps: for q1 and minus q2

        for &mut (p, ref mut coeffs) in &mut pairs {
            ell(&mut f, coeffs.next().unwrap(), &p.0);
        }

        for &mut (p, ref mut coeffs) in &mut pairs {
            ell(&mut f, coeffs.next().unwrap(), &p.0);
        }

        for &mut (_p, ref mut coeffs) in &mut pairs {
            assert_eq!(coeffs.next(), None);
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
                }

                let x = BN_U;

                let mut fp = r;
                fp.frobenius_map(1);

	            let mut fp2 = r;
                fp2.frobenius_map(2);
                let mut fp3 = fp2;
                fp3.frobenius_map(1);

                let mut fu = r;
                exp_by_x(&mut fu, x);

                let mut fu2 = fu;
                exp_by_x(&mut fu2, x);

                let mut fu3 = fu2;
                exp_by_x(&mut fu3, x);

                let mut y3 = fu;
                y3.frobenius_map(1);

                let mut fu2p = fu2;
                fu2p.frobenius_map(1);

                let mut fu3p = fu3;
                fu3p.frobenius_map(1);

                let mut y2 = fu2;
                y2.frobenius_map(2);

                let mut y0 = fp;
                y0.mul_assign(&fp2);
                y0.mul_assign(&fp3);

                let mut y1 = r;
                y1.conjugate();

                let mut y5 = fu2;
                y5.conjugate();

                y3.conjugate();

                let mut y4 = fu;
                y4.mul_assign(&fu2p);
                y4.conjugate();

                let mut y6 = fu3;
                y6.mul_assign(&fu3p);
                y6.conjugate();


                y6.square();
                y6.mul_assign(&y4);
                y6.mul_assign(&y5);

                let mut t1 = y3;
                t1.mul_assign(&y5);
                t1.mul_assign(&y6);

                y6.mul_assign(&y2);

                t1.square();
                t1.mul_assign(&y6);
                t1.square();

                let mut t0 = t1;
                t0.mul_assign(&y1);

                t1.mul_assign(&y0);

                t0.square();
                t0.mul_assign(&t1);

                Some(t0)
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

            // up to here everything was by algorith, line 11
            // use R instead of new T

            // tmp3 is the first part of line 12
            tmp3 = tmp4;
            tmp3.mul_assign(&zsquared);
            tmp3.double();
            tmp3.negate();

            // tmp6 is from line 14
            tmp6.square();
            tmp6.sub_assign(&tmp0);
            tmp6.sub_assign(&tmp5);

            tmp1.double();
            tmp1.double();

            tmp6.sub_assign(&tmp1);

            // tmp0 is the first part of line 16
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

            // t0 corresponds to line 1
            let mut t0 = zsquared;
            t0.mul_assign(&q.x);

            // t1 corresponds to lines 2 and 3
            let mut t1 = q.y;
            t1.add_assign(&r.z);
            t1.square();
            t1.sub_assign(&ysquared);
            t1.sub_assign(&zsquared);
            t1.mul_assign(&zsquared);

            // t2 corresponds to line 4
            let mut t2 = t0;
            t2.sub_assign(&r.x);

            // t3 corresponds to line 5
            let mut t3 = t2;
            t3.square();

            // t4 corresponds to line 6
            let mut t4 = t3;
            t4.double();
            t4.double();

            // t5 corresponds to line 7
            let mut t5 = t4;
            t5.mul_assign(&t2);

            // t6 corresponds to line 8
            let mut t6 = t1;
            t6.sub_assign(&r.y);
            t6.sub_assign(&r.y);

            // t9 corresponds to line 9
            let mut t9 = t6;
            t9.mul_assign(&q.x);

            // corresponds to line 10
            let mut t7 = t4;
            t7.mul_assign(&r.x);

            // corresponds to line 11, but assigns to r.x instead of T.x
            r.x = t6;
            r.x.square();
            r.x.sub_assign(&t5);
            r.x.sub_assign(&t7);
            r.x.sub_assign(&t7);

            // corresponds to line 12, but assigns to r.z instead of T.z
            r.z.add_assign(&t2);
            r.z.square();
            r.z.sub_assign(&zsquared);
            r.z.sub_assign(&t3);

            // corresponds to line 13
            let mut t10 = q.y;
            t10.add_assign(&r.z);

            // corresponds to line 14
            let mut t8 = t7;
            t8.sub_assign(&r.x);
            t8.mul_assign(&t6);

            // corresponds to line 15
            t0 = r.y;
            t0.mul_assign(&t5);
            t0.double();

            // corresponds to line 12, but assigns to r.y instead of T.y
            r.y = t8;
            r.y.sub_assign(&t0);

            // corresponds to line 17
            t10.square();
            t10.sub_assign(&ysquared);

            let mut ztsquared = r.z;
            ztsquared.square();
            
            t10.sub_assign(&ztsquared);

            // corresponds to line 18
            t9.double();
            t9.sub_assign(&t10);

            // t10 = 2*Zt from Algo 27, line 19
            t10 = r.z;
            t10.double();

            // t1 = first multiplicator of line 21
            t6.negate();

            t1 = t6;
            t1.double();

            // t9 corresponds to t9 from Algo 27
            (t10, t1, t9)
        }

        let mut coeffs = vec![];
        let mut r: G2 = q.into();

        let mut negq = q;
        negq.negate();

        for i in (1..SIX_U_PLUS_2_NAF.len()).rev() {
            coeffs.push(doubling_step(& mut r));
            let x = SIX_U_PLUS_2_NAF[i-1];
            match x {
                1 => {
                        coeffs.push(addition_step(&mut r, &q));
                    }
                -1 => {
                        coeffs.push(addition_step(&mut r, &negq));
                    }
                _ => continue,
            }
        }

        let mut q1 = q;

        q1.x.c1.negate();
        q1.x.mul_assign(&FROBENIUS_COEFF_FQ6_C1[1]);

        q1.y.c1.negate();
        q1.y.mul_assign(&XI_TO_Q_MINUS_1_OVER_2);

        coeffs.push(addition_step(&mut r, &q1));

        let mut minusq2 = q;
        minusq2.x.mul_assign(&FROBENIUS_COEFF_FQ6_C1[2]);

        coeffs.push(addition_step(&mut r, &minusq2));

        G2Prepared {
            coeffs,
            infinity: false,
        }
    }
}


#[cfg(test)]
use rand::{Rand, SeedableRng, XorShiftRng};

#[test]
fn test_pairing() {
    use {CurveProjective};
    let mut g1 = G1::one();
    
    let mut g2 = G2::one();
    g2.double();

    let pair12 = Bn256::pairing(g1, g2);

    g1 = G1::one();
    g1.double();

    g2 = G2::one();

    let pair21 = Bn256::pairing(g1, g2);

    assert_eq!(pair12, pair21);

    // print!("GT = {}\n", pair12);

    g1 = G1::one();
    g1.double();
    g1.double();

    let pair41 = Bn256::pairing(g1, g2);

    g1 = G1::one();
    g1.double();

    g2.double();

    let pair22 = Bn256::pairing(g1, g2);

    assert_eq!(pair41, pair22);

    g1 = G1::one();
    g1.double();
    g1.add_assign(&G1::one());

    g2 = G2::one();
    g2.double();

    let pair32 = Bn256::pairing(g1, g2);

    g2 = G2::one();
    g2.double();
    g2.add_assign(&G2::one());

    g1 = G1::one();
    g1.double();

    let pair23 = Bn256::pairing(g1, g2);

    assert_eq!(pair23, pair32);

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let a = Fr::rand(&mut rng);
        let b = Fr::rand(&mut rng);

        let mut g1 = G1::one();
        g1.mul_assign(a);

        let mut g2 = G2::one();
        g1.mul_assign(b);

        let pair_ab = Bn256::pairing(g1, g2);

        g1 = G1::one();
        g1.mul_assign(b);

        g2 = G2::one();
        g1.mul_assign(a);

        let pair_ba = Bn256::pairing(g1, g2);

        assert_eq!(pair_ab, pair_ba);

    }

}

#[test]
fn random_bilinearity_tests() {
    use {CurveProjective};
    use ff::PrimeField;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let mut a = G1::one();
        let ka = Fr::rand(&mut rng);
        a.mul_assign(ka);
        let mut b = G2::one();
        let kb = Fr::rand(&mut rng);
        b.mul_assign(kb);

        let c = Fr::rand(&mut rng);
        let d = Fr::rand(&mut rng);

        let mut ac = a;
        ac.mul_assign(c);

        let mut ad = a;
        ad.mul_assign(d);

        let mut bc = b;
        bc.mul_assign(c);

        let mut bd = b;
        bd.mul_assign(d);

        let acbd = Bn256::pairing(ac, bd);
        let adbc = Bn256::pairing(ad, bc);

        let mut cd = c;
        cd.mul_assign(&d);

        let abcd = Bn256::pairing(a, b).pow(cd.into_repr());

        assert_eq!(acbd, adbc);
        assert_eq!(acbd, abcd);
    }
}

#[test]
fn bn256_engine_tests() {
    ::tests::engine::engine_tests::<Bn256>();
}
