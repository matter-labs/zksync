mod ec;
mod fq;
mod fq12;
mod fq2;
mod fr;

use rand::{Rand, SeedableRng, XorShiftRng};

use pairing::bls12_381::*;
use pairing::{CurveAffine, Engine};

#[bench]
fn bench_pairing_g1_preparation(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<G1> = (0..SAMPLES).map(|_| G1::rand(&mut rng)).collect();

    let mut count = 0;
    b.iter(|| {
        let tmp = G1Affine::from(v[count]).prepare();
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_pairing_g2_preparation(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<G2> = (0..SAMPLES).map(|_| G2::rand(&mut rng)).collect();

    let mut count = 0;
    b.iter(|| {
        let tmp = G2Affine::from(v[count]).prepare();
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_pairing_miller_loop(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<(G1Prepared, G2Prepared)> = (0..SAMPLES)
        .map(|_| {
            (
                G1Affine::from(G1::rand(&mut rng)).prepare(),
                G2Affine::from(G2::rand(&mut rng)).prepare(),
            )
        })
        .collect();

    let mut count = 0;
    b.iter(|| {
        let tmp = Bls12::miller_loop(&[(&v[count].0, &v[count].1)]);
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_pairing_final_exponentiation(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<Fq12> = (0..SAMPLES)
        .map(|_| {
            (
                G1Affine::from(G1::rand(&mut rng)).prepare(),
                G2Affine::from(G2::rand(&mut rng)).prepare(),
            )
        })
        .map(|(ref p, ref q)| Bls12::miller_loop(&[(p, q)]))
        .collect();

    let mut count = 0;
    b.iter(|| {
        let tmp = Bls12::final_exponentiation(&v[count]);
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_pairing_full(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<(G1, G2)> = (0..SAMPLES)
        .map(|_| (G1::rand(&mut rng), G2::rand(&mut rng)))
        .collect();

    let mut count = 0;
    b.iter(|| {
        let tmp = Bls12::pairing(v[count].0, v[count].1);
        count = (count + 1) % SAMPLES;
        tmp
    });
}
