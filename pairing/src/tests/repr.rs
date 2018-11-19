use ff::PrimeFieldRepr;
use rand::{SeedableRng, XorShiftRng};

pub fn random_repr_tests<R: PrimeFieldRepr>() {
    random_encoding_tests::<R>();
    random_shl_tests::<R>();
    random_shr_tests::<R>();
}

fn random_encoding_tests<R: PrimeFieldRepr>() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..1000 {
        let r = R::rand(&mut rng);

        // Big endian
        {
            let mut rdecoded = R::default();

            let mut v: Vec<u8> = vec![];
            r.write_be(&mut v).unwrap();
            rdecoded.read_be(&v[0..]).unwrap();

            assert_eq!(r, rdecoded);
        }

        // Little endian
        {
            let mut rdecoded = R::default();

            let mut v: Vec<u8> = vec![];
            r.write_le(&mut v).unwrap();
            rdecoded.read_le(&v[0..]).unwrap();

            assert_eq!(r, rdecoded);
        }

        {
            let mut rdecoded_le = R::default();
            let mut rdecoded_be_flip = R::default();

            let mut v: Vec<u8> = vec![];
            r.write_le(&mut v).unwrap();

            // This reads in little-endian, so we are done.
            rdecoded_le.read_le(&v[..]).unwrap();

            // This reads in big-endian, so we perform a swap of the
            // bytes beforehand.
            let v: Vec<u8> = v.into_iter().rev().collect();
            rdecoded_be_flip.read_be(&v[..]).unwrap();

            assert_eq!(rdecoded_le, rdecoded_be_flip);
        }
    }
}

fn random_shl_tests<R: PrimeFieldRepr>() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..100 {
        let r = R::rand(&mut rng);

        for shift in 0..(r.num_bits() + 1) {
            let mut r1 = r;
            let mut r2 = r;

            for _ in 0..shift {
                r1.mul2();
            }

            r2.shl(shift);

            assert_eq!(r1, r2);
        }
    }
}

fn random_shr_tests<R: PrimeFieldRepr>() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    for _ in 0..100 {
        let r = R::rand(&mut rng);

        for shift in 0..(r.num_bits() + 1) {
            let mut r1 = r;
            let mut r2 = r;

            for _ in 0..shift {
                r1.div2();
            }

            r2.shr(shift);

            assert_eq!(r1, r2);
        }
    }
}
