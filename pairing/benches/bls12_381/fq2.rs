use rand::{Rand, SeedableRng, XorShiftRng};

use ff::{Field, SqrtField};
use pairing::bls12_381::*;

#[bench]
fn bench_fq2_add_assign(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<(Fq2, Fq2)> = (0..SAMPLES)
        .map(|_| (Fq2::rand(&mut rng), Fq2::rand(&mut rng)))
        .collect();

    let mut count = 0;
    b.iter(|| {
        let mut tmp = v[count].0;
        tmp.add_assign(&v[count].1);
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_fq2_sub_assign(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<(Fq2, Fq2)> = (0..SAMPLES)
        .map(|_| (Fq2::rand(&mut rng), Fq2::rand(&mut rng)))
        .collect();

    let mut count = 0;
    b.iter(|| {
        let mut tmp = v[count].0;
        tmp.sub_assign(&v[count].1);
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_fq2_mul_assign(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<(Fq2, Fq2)> = (0..SAMPLES)
        .map(|_| (Fq2::rand(&mut rng), Fq2::rand(&mut rng)))
        .collect();

    let mut count = 0;
    b.iter(|| {
        let mut tmp = v[count].0;
        tmp.mul_assign(&v[count].1);
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_fq2_squaring(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<Fq2> = (0..SAMPLES).map(|_| Fq2::rand(&mut rng)).collect();

    let mut count = 0;
    b.iter(|| {
        let mut tmp = v[count];
        tmp.square();
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_fq2_inverse(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<Fq2> = (0..SAMPLES).map(|_| Fq2::rand(&mut rng)).collect();

    let mut count = 0;
    b.iter(|| {
        let tmp = v[count].inverse();
        count = (count + 1) % SAMPLES;
        tmp
    });
}

#[bench]
fn bench_fq2_sqrt(b: &mut ::test::Bencher) {
    const SAMPLES: usize = 1000;

    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let v: Vec<Fq2> = (0..SAMPLES).map(|_| Fq2::rand(&mut rng)).collect();

    let mut count = 0;
    b.iter(|| {
        let tmp = v[count].sqrt();
        count = (count + 1) % SAMPLES;
        tmp
    });
}
