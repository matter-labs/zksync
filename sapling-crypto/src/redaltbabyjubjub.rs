//! Implementation of RedJubjub, a specialization of RedDSA to the Jubjub curve.
//! See section 5.4.6 of the Sapling protocol specification.

use ff::{Field, PrimeField, PrimeFieldRepr};
use rand::{Rng, Rand};
use std::io::{self, Read, Write};

// use jubjub::{FixedGenerators, JubjubEngine, JubjubParams, Unknown, edwards::Point};
use util::{hash_to_scalar};

use redjubjub::{
    PrivateKey, 
    PublicKey,
    BatchEntry,
    batch_verify,
    Signature
};

#[cfg(test)]
mod tests {
    use pairing::bn256::Bn256;
    use rand::thread_rng;

    use alt_babyjubjub::{AltJubjubBn256, fs::Fs, edwards, FixedGenerators};

    use super::*;

    #[test]
    fn test_batch_verify() {
        let rng = &mut thread_rng();
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;

        let sk1 = PrivateKey::<Bn256>(rng.gen());
        let vk1 = PublicKey::from_private(&sk1, p_g, params);
        let msg1 = b"Foo bar";
        let sig1 = sk1.sign(msg1, rng, p_g, params);
        assert!(vk1.verify(msg1, &sig1, p_g, params));

        let sk2 = PrivateKey::<Bn256>(rng.gen());
        let vk2 = PublicKey::from_private(&sk2, p_g, params);
        let msg2 = b"Foo bar";
        let sig2 = sk2.sign(msg2, rng, p_g, params);
        assert!(vk2.verify(msg2, &sig2, p_g, params));

        let mut batch = vec![
            BatchEntry { vk: vk1, msg: msg1, sig: sig1 },
            BatchEntry { vk: vk2, msg: msg2, sig: sig2 }
        ];

        assert!(batch_verify(rng, &batch, p_g, params));

        batch[0].sig = sig2;

        assert!(!batch_verify(rng, &batch, p_g, params));
    }

    #[test]
    fn cofactor_check() {
        let rng = &mut thread_rng();
        let params = &AltJubjubBn256::new();
        let zero = edwards::Point::zero();
        let p_g = FixedGenerators::SpendingKeyGenerator;

        // Get a point of order 8
        let p8 = loop {
            let r = edwards::Point::<Bn256, _>::rand(rng, params).mul(Fs::char(), params);

            let r2 = r.double(params);
            let r4 = r2.double(params);
            let r8 = r4.double(params);

            if r2 != zero && r4 != zero && r8 == zero {
                break r;
            }
        };

        let sk = PrivateKey::<Bn256>(rng.gen());
        let vk = PublicKey::from_private(&sk, p_g, params);

        // TODO: This test will need to change when #77 is fixed
        let msg = b"Foo bar";
        let sig = sk.sign(msg, rng, p_g, params);
        assert!(vk.verify(msg, &sig, p_g, params));

        let vktorsion = PublicKey(vk.0.add(&p8, params));
        assert!(vktorsion.verify(msg, &sig, p_g, params));
    }

    #[test]
    fn round_trip_serialization() {
        let rng = &mut thread_rng();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let params = &AltJubjubBn256::new();

        for _ in 0..1000 {
            let sk = PrivateKey::<Bn256>(rng.gen());
            let vk = PublicKey::from_private(&sk, p_g, params);
            let msg = b"Foo bar";
            let sig = sk.sign(msg, rng, p_g, params);

            let mut sk_bytes = [0u8; 32];
            let mut vk_bytes = [0u8; 32];
            let mut sig_bytes = [0u8; 64];
            sk.write(&mut sk_bytes[..]).unwrap();
            vk.write(&mut vk_bytes[..]).unwrap();
            sig.write(&mut sig_bytes[..]).unwrap();

            let sk_2 = PrivateKey::<Bn256>::read(&sk_bytes[..]).unwrap();
            let vk_2 = PublicKey::from_private(&sk_2, p_g, params);
            let mut vk_2_bytes = [0u8; 32];
            vk_2.write(&mut vk_2_bytes[..]).unwrap();
            assert!(vk_bytes == vk_2_bytes);

            let vk_2 = PublicKey::<Bn256>::read(&vk_bytes[..], params).unwrap();
            let sig_2 = Signature::read(&sig_bytes[..]).unwrap();
            assert!(vk.verify(msg, &sig_2, p_g, params));
            assert!(vk_2.verify(msg, &sig, p_g, params));
            assert!(vk_2.verify(msg, &sig_2, p_g, params));
        }
    }

    #[test]
    fn random_signatures() {
        let rng = &mut thread_rng();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let params = &AltJubjubBn256::new();

        for _ in 0..1000 {
            let sk = PrivateKey::<Bn256>(rng.gen());
            let vk = PublicKey::from_private(&sk, p_g, params);

            let msg1 = b"Foo bar";
            let msg2 = b"Spam eggs";

            let sig1 = sk.sign(msg1, rng, p_g, params);
            let sig2 = sk.sign(msg2, rng, p_g, params);

            assert!(vk.verify(msg1, &sig1, p_g, params));
            assert!(vk.verify(msg2, &sig2, p_g, params));
            assert!(!vk.verify(msg1, &sig2, p_g, params));
            assert!(!vk.verify(msg2, &sig1, p_g, params));

            let alpha = rng.gen();
            let rsk = sk.randomize(alpha);
            let rvk = vk.randomize(alpha, p_g, params);

            let sig1 = rsk.sign(msg1, rng, p_g, params);
            let sig2 = rsk.sign(msg2, rng, p_g, params);

            assert!(rvk.verify(msg1, &sig1, p_g, params));
            assert!(rvk.verify(msg2, &sig2, p_g, params));
            assert!(!rvk.verify(msg1, &sig2, p_g, params));
            assert!(!rvk.verify(msg2, &sig1, p_g, params));
        }
    }
}
