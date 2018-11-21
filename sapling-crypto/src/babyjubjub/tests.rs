use super::{
    JubjubEngine, 
    JubjubParams, 
    PrimeOrder, 
    Unknown,
    montgomery,
    edwards
};

use ff::{
    Field,
    PrimeField,
    PrimeFieldRepr,
    SqrtField,
    LegendreSymbol
};

use rand::{XorShiftRng, SeedableRng, Rand};

pub fn test_suite<E: JubjubEngine>(params: &E::Params) {
    test_back_and_forth::<E>(params);
    // test_jubjub_params::<E>(params);
    test_rand::<E>(params);
    test_get_for::<E>(params);
    test_identities::<E>(params);
    test_addition_associativity::<E>(params);
    test_order::<E>(params);
    test_mul_associativity::<E>(params);
    test_loworder::<E>(params);
    test_read_write::<E>(params);
}

fn is_on_mont_curve<E: JubjubEngine, P: JubjubParams<E>>(
    x: E::Fr,
    y: E::Fr,
    params: &P
) -> bool
{
    let mut lhs = y;
    lhs.square();

    let mut x2 = x;
    x2.square();

    let mut x3 = x2;
    x3.mul_assign(&x);

    let mut rhs = x2;
    rhs.mul_assign(params.montgomery_a());
    rhs.add_assign(&x);
    rhs.add_assign(&x3);

    lhs == rhs
}

fn is_on_twisted_edwards_curve<E: JubjubEngine, P: JubjubParams<E>>(
    x: E::Fr,
    y: E::Fr,
    params: &P
) -> bool
{
    let mut x2 = x;
    x2.square();

    let mut y2 = y;
    y2.square();

    // a*x^2 + y^2
    let mut lhs = y2;
    let mut a_x2 = x2;
    a_x2.mul_assign(params.edwards_a());
    lhs.add_assign(&a_x2);

    // 1 + d x^2 y^2
    let mut rhs = y2;
    rhs.mul_assign(&x2);
    rhs.mul_assign(params.edwards_d());
    rhs.add_assign(&E::Fr::one());

    lhs == rhs
}

fn test_loworder<E: JubjubEngine>(params: &E::Params) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let inf = montgomery::Point::zero();

    // try to find a point of order 8
    let p = loop {
        let r = montgomery::Point::<E, _>::rand(rng, params).mul(E::Fs::char(), params);

        let r2 = r.double(params);
        let r4 = r2.double(params);
        let r8 = r4.double(params);

        if r2 != inf && r4 != inf && r8 == inf {
            break r;
        }
    };

    let mut loworder_points = vec![];
    {
        let mut tmp = p.clone();

        for _ in 0..8 {
            assert!(!loworder_points.contains(&tmp));
            loworder_points.push(tmp.clone());
            tmp = tmp.add(&p, params);
        }
    }
    assert!(loworder_points[7] == inf);
}

fn test_mul_associativity<E: JubjubEngine>(params: &E::Params) {
    use self::edwards::Point;
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..100 {
        // Pick a random point and multiply it by the cofactor
        let base = Point::<E, _>::rand(rng, params).mul_by_cofactor(params);

        let mut a = E::Fs::rand(rng);
        let b = E::Fs::rand(rng);
        let c = E::Fs::rand(rng);

        let res1 = base.mul(a, params).mul(b, params).mul(c, params);
        let res2 = base.mul(b, params).mul(c, params).mul(a, params);
        let res3 = base.mul(c, params).mul(a, params).mul(b, params);
        a.mul_assign(&b);
        a.mul_assign(&c);
        let res4 = base.mul(a, params);

        assert!(res1 == res2);
        assert!(res2 == res3);
        assert!(res3 == res4);

        let (x, y) = res1.into_xy();
        assert!(is_on_twisted_edwards_curve(x, y, params));

        let (x, y) = res2.into_xy();
        assert!(is_on_twisted_edwards_curve(x, y, params));

        let (x, y) = res3.into_xy();
        assert!(is_on_twisted_edwards_curve(x, y, params));
    }
}

fn test_order<E: JubjubEngine>(params: &E::Params) {
    use self::edwards::Point;
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    // The neutral element is in the prime order subgroup.
    assert!(Point::<E, PrimeOrder>::zero().as_prime_order(params).is_some());

    for _ in 0..50 {
        // Pick a random point and multiply it by the cofactor
        let base = Point::<E, _>::rand(rng, params).mul_by_cofactor(params);

        // Any point multiplied by the cofactor will be in the prime
        // order subgroup
        assert!(base.as_prime_order(params).is_some());
    }

    // It's very likely that at least one out of 50 random points on the curve
    // is not in the prime order subgroup.
    let mut at_least_one_not_in_prime_order_subgroup = false;
    for _ in 0..50 {
        // Pick a random point.
        let base = Point::<E, _>::rand(rng, params);

        at_least_one_not_in_prime_order_subgroup |= base.as_prime_order(params).is_none();
    }
    assert!(at_least_one_not_in_prime_order_subgroup);
}

fn test_addition_associativity<E: JubjubEngine>(params: &E::Params) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        use self::montgomery::Point;

        let a = Point::<E, _>::rand(rng, params);
        let b = Point::<E, _>::rand(rng, params);
        let c = Point::<E, _>::rand(rng, params);

        assert!(a.add(&b, &params).add(&c, &params) == c.add(&a, &params).add(&b, &params));
    }

    for _ in 0..1000 {
        use self::edwards::Point;

        let a = Point::<E, _>::rand(rng, params);
        let b = Point::<E, _>::rand(rng, params);
        let c = Point::<E, _>::rand(rng, params);

        assert!(a.add(&b, &params).add(&c, &params) == c.add(&a, &params).add(&b, &params));
    }
}

fn test_identities<E: JubjubEngine>(params: &E::Params) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    {
        use self::edwards::Point;

        let z = Point::<E, PrimeOrder>::zero();
        assert!(z.double(&params) == z);
        assert!(z.negate() == z);

        for _ in 0..100 {
            let r = Point::<E, _>::rand(rng, params);

            assert!(r.add(&Point::zero(), &params) == r);
            assert!(r.add(&r.negate(), &params) == Point::zero());
        }
    }

    {
        use self::montgomery::Point;

        let z = Point::<E, PrimeOrder>::zero();
        assert!(z.double(&params) == z);
        assert!(z.negate() == z);

        for _ in 0..100 {
            let r = Point::<E, _>::rand(rng, params);

            assert!(r.add(&Point::zero(), &params) == r);
            assert!(r.add(&r.negate(), &params) == Point::zero());
        }
    }
}

fn test_get_for<E: JubjubEngine>(params: &E::Params) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let y = E::Fr::rand(rng);
        let sign = bool::rand(rng);

        if let Some(mut p) = edwards::Point::<E, _>::get_for_y(y, sign, params) {
            assert!(p.into_xy().0.into_repr().is_odd() == sign);
            p = p.negate();
            assert!(
                edwards::Point::<E, _>::get_for_y(y, !sign, params).unwrap()
                ==
                p
            );
        }
    }
}

fn test_read_write<E: JubjubEngine>(params: &E::Params) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let e = edwards::Point::<E, _>::rand(rng, params);

        let mut v = vec![];
        e.write(&mut v).unwrap();

        let e2 = edwards::Point::read(&v[..], params).unwrap();

        assert!(e == e2);
    }
}

fn test_rand<E: JubjubEngine>(params: &E::Params) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let p = montgomery::Point::<E, _>::rand(rng, params);
        let e = edwards::Point::<E, _>::rand(rng, params);

        {
            let (x, y) = p.into_xy().unwrap();
            assert!(is_on_mont_curve(x, y, params));
        }

        {
            let (x, y) = e.into_xy();
            assert!(is_on_twisted_edwards_curve(x, y, params));
        }
    }
}

fn test_back_and_forth<E: JubjubEngine>(params: &E::Params) {
    let rng = &mut XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let s = E::Fs::rand(rng);
        let edwards_p1 = edwards::Point::<E, _>::rand(rng, params);
        let mont_p1 = montgomery::Point::from_edwards(&edwards_p1, params);
        let mont_p2 = montgomery::Point::<E, _>::rand(rng, params);
        let edwards_p2 = edwards::Point::from_montgomery(&mont_p2, params);

        let mont = mont_p1.add(&mont_p2, params).mul(s, params);
        let edwards = edwards_p1.add(&edwards_p2, params).mul(s, params);

        assert!(
            montgomery::Point::from_edwards(&edwards, params) == mont
        );

        assert!(
            edwards::Point::from_montgomery(&mont, params) == edwards
        );
    }
}

pub fn test_jubjub_params<E: JubjubEngine>(params: &E::Params) {
    // a
    let a = *params.edwards_a();

    {
        // Check that 2A is consistent with A
        let mut tmp = *params.montgomery_a();
        tmp.double();

        assert_eq!(&tmp, params.montgomery_2a());
    }

    {
        // The twisted Edwards addition law is complete when d is nonsquare
        // and a is square.

        assert!(params.edwards_d().legendre() == LegendreSymbol::QuadraticNonResidue);
        assert!(a.legendre() == LegendreSymbol::QuadraticResidue);
    }

    {
        // Other convenient sanity checks regarding d

        // tmp = d
        let mut tmp = *params.edwards_d();

        // 1 / d is nonsquare
        assert!(tmp.inverse().unwrap().legendre() == LegendreSymbol::QuadraticNonResidue);

        // tmp = -d
        tmp.negate();

        // -d is nonsquare
        assert!(tmp.legendre() == LegendreSymbol::QuadraticNonResidue);

        // 1 / -d is nonsquare
        assert!(tmp.inverse().unwrap().legendre() == LegendreSymbol::QuadraticNonResidue);
    }

    {
        // Check that A^2 - 4 is nonsquare:
        let mut tmp = params.montgomery_a().clone();
        tmp.square();
        tmp.sub_assign(&E::Fr::from_str("4").unwrap());
        assert!(tmp.legendre() == LegendreSymbol::QuadraticNonResidue);
    }

    {
        // Check that A - 2 is nonsquare:
        let mut tmp = params.montgomery_a().clone();
        tmp.sub_assign(&E::Fr::from_str("2").unwrap());
        assert!(tmp.legendre() == LegendreSymbol::QuadraticNonResidue);
    }

    {
        // Check the validity of the scaling factor
        let mut tmp = *params.edwards_a();
        tmp.sub_assign(&params.edwards_d());
        tmp = tmp.inverse().unwrap();
        tmp.mul_assign(&E::Fr::from_str("4").unwrap());
        tmp = tmp.sqrt().unwrap();
        assert_eq!(&tmp, params.scale());
    }

    {
        // Check that the number of windows for fixed-base
        // scalar multiplication is sufficient for all scalars.

        assert!(params.fixed_base_chunks_per_generator() * 3 >= E::Fs::NUM_BITS as usize);

        // ... and that it's *just* efficient enough.

        assert!((params.fixed_base_chunks_per_generator() - 1) * 3 < E::Fs::NUM_BITS as usize);
    }

    {
        // Check that the number of windows per generator
        // in the Pedersen hash does not allow for collisions

        let mut cur = E::Fs::one().into_repr();

        let mut max = E::Fs::char();
        {
            max.sub_noborrow(&E::Fs::one().into_repr());
            max.div2();
        }

        let mut pacc = E::Fs::zero().into_repr();
        let mut nacc = E::Fs::char();

        for _ in 0..params.pedersen_hash_chunks_per_generator()
        {
            // tmp = cur * 4
            let mut tmp = cur;
            tmp.mul2();
            tmp.mul2();

            pacc.add_nocarry(&tmp);
            nacc.sub_noborrow(&tmp);

            assert!(pacc < max);
            assert!(pacc < nacc);

            // cur = cur * 16
            for _ in 0..4 {
                cur.mul2();
            }
        }
    }
}
