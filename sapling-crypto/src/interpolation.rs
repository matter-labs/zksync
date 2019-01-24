use pairing::Engine;

use ff::{
    Field,
    PrimeField
};

/// Perform a Lagrange interpolation for a set of points
/// It's O(n^2) operations, so use with caution
pub fn interpolate<E: Engine>(
    points: &[(E::Fr, E::Fr)]
) -> Option<Vec<E::Fr>> {
    let max_degree_plus_one = points.len();
    assert!(max_degree_plus_one >= 2, "should interpolate for degree >= 1");
    let external_iter = points.clone().into_iter();
    let internal = points.clone();
    let mut coeffs = vec![E::Fr::zero(); max_degree_plus_one];
    for (k, p_k) in external_iter.enumerate() {
        let (x_k, y_k) = p_k;
        // coeffs from 0 to max_degree - 1
        let mut contribution = vec![E::Fr::zero(); max_degree_plus_one];
        let mut demoninator = E::Fr::one();
        let mut max_contribution_degree = 0;
        for (j, p_j) in internal.iter().enumerate() {
            let (x_j, _) = p_j;
            if j == k {
                continue;
            }

            let mut diff = x_k.clone();
            diff.sub_assign(&x_j);
            demoninator.mul_assign(&diff);

            if max_contribution_degree == 0 {
                max_contribution_degree = 1;
                contribution.get_mut(0).expect("must have enough coefficients").sub_assign(&x_j);
                contribution.get_mut(1).expect("must have enough coefficients").add_assign(&E::Fr::one());
            } else {
                let mul_by_minus_x_j: Vec<E::Fr> = contribution.iter().map(|el| {
                    let mut tmp = el.clone();
                    tmp.mul_assign(&x_j);
                    tmp.negate();

                    tmp
                }).collect();

                contribution.insert(0, E::Fr::zero());
                contribution.truncate(max_degree_plus_one);

                assert_eq!(mul_by_minus_x_j.len(), max_degree_plus_one);
                for (i, c) in contribution.iter_mut().enumerate() {
                    let other = mul_by_minus_x_j.get(i).expect("should have enough elements");
                    c.add_assign(&other);
                }
            }
        }

        demoninator = demoninator.inverse().expect("denominator must be non-zero");
        for (i, this_contribution) in contribution.into_iter().enumerate() {
            let c = coeffs.get_mut(i).expect("should have enough coefficients");
            let mut tmp = this_contribution;
            tmp.mul_assign(&demoninator);
            tmp.mul_assign(&y_k);
            c.add_assign(&tmp);
        }

    }

    Some(coeffs)
}

pub fn evaluate_at_x<E: Engine>(
    coeffs: &[E::Fr],
    x: &E::Fr
) -> E::Fr {
    let mut res = E::Fr::zero();
    let mut pow = E::Fr::one();
    for c in coeffs.iter() {
        let mut tmp = c.clone();
        tmp.mul_assign(&pow);
        res.add_assign(&tmp);

        pow.mul_assign(&x);
    }

    res
}

#[test]
fn test_interpolation_1(){
    use pairing::bn256::{Bn256, Fr};
    let points = vec![(Fr::zero(), Fr::one()), (Fr::one(), Fr::from_str("2").unwrap())];
    let interpolation_res = interpolate::<Bn256>(&points[..]).expect("must interpolate a linear func");
    assert_eq!(interpolation_res.len(), 2);
    for (i, c) in interpolation_res.iter().enumerate() {
        println!("Coeff {} = {}", i, c);
    }

    for (i, p) in points.iter().enumerate() {
        let (x, y) = p;
        let val = evaluate_at_x::<Bn256>(&interpolation_res[..], &x);
        assert_eq!(*y, val);
        println!("Eval at {} = {}, original value = {}", x, val, y);
    }
}

#[test]
fn test_interpolation_powers_of_2(){
    use pairing::bn256::{Bn256, Fr};
    const MAX_POWER: u32 = Fr::CAPACITY;

    let mut points: Vec<(Fr, Fr)> = vec![];
    let mut power = Fr::one();
    let two = Fr::from_str("2").unwrap();
    for i in 0..MAX_POWER {
        let x = Fr::from_str(&i.to_string()).unwrap();
        let y = power.clone();
        points.push((x,y));

        power.mul_assign(&two);
    }
    let interpolation_res = interpolate::<Bn256>(&points[..]).expect("must interpolate");
    assert_eq!(*interpolation_res.get(0).unwrap(), Fr::one());
    assert_eq!(interpolation_res.len(), points.len(), "array sized must match");
    assert_eq!(interpolation_res.len(), MAX_POWER as usize, "array size must be equal to the max power");

    for (i, p) in points.iter().enumerate() {
        let (x, y) = p;
        let val = evaluate_at_x::<Bn256>(&interpolation_res[..], &x);
        // println!("Eval at {} = {}, original value = {}", x, val, y);
        // assert!(*y == val, format!("must assert equality for x = {}", x) );
        assert_eq!(*y, val);

    }
}


