use pairing::{
    Engine,
};

use ff::{Field};

use bellman::{
    SynthesisError,
    ConstraintSystem
};

use super::{
    Assignment
};

use super::num::{
    AllocatedNum,
};

use ::jubjub::{
    edwards,
    JubjubEngine,
    JubjubParams,
    FixedGenerators
};

use super::lookup::{
    lookup3_xy
};

use super::boolean::{
    Boolean, 
    field_into_boolean_vec_le,
};

use super::ecc::EdwardsPoint;

use super::blake2s::{blake2s};

#[derive(Clone)]
pub struct EddsaSignature<E: JubjubEngine> {
    pub r: EdwardsPoint<E>,
    pub s: AllocatedNum<E>,
    pub pk: EdwardsPoint<E>
}

use ::alt_babyjubjub::{fs::Fs};

use constants::{MATTER_EDDSA_BLAKE2S_PERSONALIZATION};

impl <E: JubjubEngine>EddsaSignature<E> {

    pub fn verify_eddsa_for_snark<CS>(
        &self,
        mut cs: CS,
        params: &E::Params,
        message: &[Boolean],
        generator: EdwardsPoint<E>
    ) -> Result<(), SynthesisError>
        where CS: ConstraintSystem<E>
    {
        // TODO check that s < Fs::Char
        let scalar_bits = field_into_boolean_vec_le(
            cs.namespace(|| "Get S bits"),
            self.s.get_value()
        )?;

        let sb = generator.mul(
            cs.namespace(|| "S*B computation"),
            &scalar_bits, params
        )?;

        // h = Hash(R_X || message)

        // only order of R is checked. Public key and generator can be guaranteed to be in proper group!
        // by some other means for out particular case
        self.r.assert_not_small_order(
            cs.namespace(|| "R is in right order"),
            &params
        )?;

        let mut hash_bits: Vec<Boolean> = vec![];

        let r_x_serialized = field_into_boolean_vec_le(
            cs.namespace(|| "Serialize R_X"), self.r.get_x().get_value()
        )?;

        hash_bits.extend(r_x_serialized.into_iter());
        hash_bits.resize(256, Boolean::Constant(false));

        hash_bits.extend(message.iter().cloned());
        hash_bits.resize(512, Boolean::Constant(false));

        assert_eq!(hash_bits.len(), 512);

        let h = blake2s(
            cs.namespace(|| "Calculate EdDSA hash"),
            &hash_bits, 
            MATTER_EDDSA_BLAKE2S_PERSONALIZATION
        )?;
        
        let pk_mul_hash = self.pk.mul(
            cs.namespace(|| "Calculate h*PK"), 
            &h, 
            params
        )?;

        let rhs = pk_mul_hash.add(
            cs.namespace(|| "Make signature RHS"), 
            &self.r, 
            params
        )?;

        let rhs_x = rhs.get_x();
        let rhs_y = rhs.get_y();

        let sb_x = sb.get_x();
        let sb_y = sb.get_y();

        let one = CS::one();
        cs.enforce(
            || "check x coordinate of signature",
            |lc| lc + rhs_x.get_variable(),
            |lc| lc + one,
            |lc| lc + sb_x.get_variable()
        );

        cs.enforce(
            || "check y coordinate of signature",
            |lc| lc + rhs_y.get_variable(),
            |lc| lc + one,
            |lc| lc + sb_y.get_variable()
        );

        return Ok(());
    }

    pub fn verify_raw_message_signature<CS>(
        &self,
        mut cs: CS,
        params: &E::Params,
        message: &[Boolean],
        generator: EdwardsPoint<E>,
        max_message_len: usize
    ) -> Result<(), SynthesisError>
        where CS: ConstraintSystem<E>
    {
        // TODO check that s < Fs::Char

        // message is always padded to 256 bits in this gadget, but still checked on synthesis
        assert!(message.len() <= max_message_len * 8);

        // let scalar_bits = AllocatedNum::alloc(
        //     cs.namespace(|| "Allocate S witness"),
        //     || Ok(*self.s.get_value().get()?)
        // )?;

        let scalar_bits = self.s.into_bits_le(
            cs.namespace(|| "Get S bits")
        )?;

        // generator.assert_not_small_order(
        //     cs.namespace(|| "Temporary check that generator is of correct order"),
        //     &params
        // )?;

        // self.pk.assert_not_small_order(
        //     cs.namespace(|| "Temporary check that public key is of correct order"),
        //     &params
        // )?;

        let sb = generator.mul(
            cs.namespace(|| "S*B computation"),
            &scalar_bits, 
            params
        )?;

        // only order of R is checked. Public key and generator can be guaranteed to be in proper group!
        // by some other means for out particular case
        self.r.assert_not_small_order(
            cs.namespace(|| "R is in right order"),
            &params
        )?;

        let mut h: Vec<Boolean> = vec![];
        h.extend(message.iter().cloned());
        h.resize(256, Boolean::Constant(false));

        assert_eq!(h.len(), 256);
        
        let pk_mul_hash = self.pk.mul(
            cs.namespace(|| "Calculate h*PK"), 
            &h, 
            params
        )?;

        let rhs = pk_mul_hash.add(
            cs.namespace(|| "Make signature RHS"), 
            &self.r, 
            params
        )?;

        let rhs_x = rhs.get_x();
        let rhs_y = rhs.get_y();

        let sb_x = sb.get_x();
        let sb_y = sb.get_y();

        let one = CS::one();
        cs.enforce(
            || "check x coordinate of signature",
            |lc| lc + rhs_x.get_variable(),
            |lc| lc + one,
            |lc| lc + sb_x.get_variable()
        );

        cs.enforce(
            || "check y coordinate of signature",
            |lc| lc + rhs_y.get_variable(),
            |lc| lc + one,
            |lc| lc + sb_y.get_variable()
        );

        return Ok(());
    }
} 


#[cfg(test)]
mod test {
    use ::eddsa::{PrivateKey, PublicKey};
    use rand::{SeedableRng, Rng, XorShiftRng};
    use super::*;
    use ::circuit::test::*;
    use ::circuit::boolean::{Boolean, AllocatedBit};
    use pairing::bn256::{Bn256, Fr};
    use ff::{PrimeField, PrimeFieldRepr};
    use ::alt_babyjubjub::AltJubjubBn256;
    
    #[test]
    fn test_valid_for_snark_signatures() {
        
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let params = &AltJubjubBn256::new();
        let mut cs = TestConstraintSystem::<Bn256>::new();
        let sk = PrivateKey::<Bn256>(rng.gen());
        let vk = PublicKey::from_private(&sk, p_g, params);

        let msg1 = b"Foo bar pad to16"; // 16 bytes

        let mut input: Vec<bool> = vec![];

        for b in msg1.iter() {  
            for i in (0..8).into_iter() {
                if (b & (1 << i)) != 0 {
                    input.extend(&[true; 1]);
                } else {
                    input.extend(&[false; 1]);
                }
            }
        }

        let sig1 = sk.sign_for_snark(msg1, &mut rng, p_g, params);
        assert!(vk.verify_for_snark(msg1, &sig1, p_g, params));

        let input_bools: Vec<Boolean> = input.iter().enumerate().map(|(i, b)| {
            Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| format!("input {}", i)), Some(*b)).unwrap()
            )
        }).collect();

        let mut sigs_bytes = [0u8; 32];
        sig1.s.into_repr().write_le(& mut sigs_bytes[..]).expect("get LE bytes of signature S");
        let mut sigs_repr = <pairing::bn256::Fr as PrimeField>::Repr::from(0);
        sigs_repr.read_le(&sigs_bytes[..]).expect("interpret S as field element representation");

        let sigs_converted = Fr::from_repr(sigs_repr).unwrap();

        let s = AllocatedNum::alloc(cs.namespace(|| "allocate s"), || {
                Ok(sigs_converted)
            }
        ).unwrap();

        let public_generator = params.generator(FixedGenerators::SpendingKeyGenerator).clone();

        let generator = EdwardsPoint::witness(cs.namespace(|| "allocate public generator"), Some(public_generator), params).unwrap();

        let r = EdwardsPoint::witness(cs.namespace(|| "allocate r"), Some(sig1.r), params).unwrap();

        let pk = EdwardsPoint::witness(cs.namespace(|| "allocate pk"), Some(vk.0), params).unwrap();

        let signature = EddsaSignature{r, s, pk};
        signature.verify_eddsa_for_snark(cs.namespace(|| "verify signature"), params, &input_bools, generator).expect("succesfully generated verifying gadget");

        assert!(cs.is_satisfied());
        print!("EdDSA variant for snark verification takes constraints: {}\n", cs.num_constraints());
    }

    #[test]
    fn test_valid_raw_message_signatures() {
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let params = &AltJubjubBn256::new();
        let mut cs = TestConstraintSystem::<Bn256>::new();
        let sk = PrivateKey::<Bn256>(rng.gen());
        let vk = PublicKey::from_private(&sk, p_g, params);

        let msg1 = b"Foo bar pad to16"; // 16 bytes

        let mut input: Vec<bool> = vec![];

        for b in msg1.iter() {  
            for i in (0..8).into_iter() {
                if (b & (1 << i)) != 0 {
                    input.extend(&[true; 1]);
                } else {
                    input.extend(&[false; 1]);
                }
            }
        }

        // test for maximum message length of 16 bytes
        let sig1 = sk.sign_raw_message(msg1, &mut rng, p_g, params, 16);
        assert!(vk.verify_for_raw_message(msg1, &sig1, p_g, params, 16));

        let input_bools: Vec<Boolean> = input.iter().enumerate().map(|(i, b)| {
            Boolean::from(
                AllocatedBit::alloc(cs.namespace(|| format!("input {}", i)), Some(*b)).unwrap()
            )
        }).collect();

        let mut sigs_bytes = [0u8; 32];
        sig1.s.into_repr().write_le(& mut sigs_bytes[..]).expect("get LE bytes of signature S");
        let mut sigs_repr = <pairing::bn256::Fr as PrimeField>::Repr::from(0);
        sigs_repr.read_le(&sigs_bytes[..]).expect("interpret S as field element representation");

        let sigs_converted = Fr::from_repr(sigs_repr).unwrap();

        let s = AllocatedNum::alloc(cs.namespace(|| "allocate s"), || {
                Ok(sigs_converted)
            }
        ).unwrap();

        let public_generator = params.generator(FixedGenerators::SpendingKeyGenerator).clone();

        let generator = EdwardsPoint::witness(cs.namespace(|| "allocate public generator"), Some(public_generator), params).unwrap();

        let r = EdwardsPoint::witness(cs.namespace(|| "allocate r"), Some(sig1.r), params).unwrap();

        let pk = EdwardsPoint::witness(cs.namespace(|| "allocate pk"), Some(vk.0), params).unwrap();

        let signature = EddsaSignature{r, s, pk};
        signature.verify_raw_message_signature(cs.namespace(|| "verify signature"), params, &input_bools, generator, 16).expect("succesfully generated verifying gadget");

        assert!(cs.is_satisfied());
        print!("EdDSA variant raw message signature takes constraints: {}\n", cs.num_constraints());
    }

}


