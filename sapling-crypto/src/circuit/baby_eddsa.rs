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
    Num
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

use super::boolean::{Boolean, 
    field_into_allocated_bits_le, 
    field_into_boolean_vec_le,
    AllocatedBit
};

use super::ecc::EdwardsPoint;

use ::redjubjub::{PublicKey};

use super::blake2s::{blake2s};

use super::pedersen_hash::{Personalization};

#[derive(Clone)]
pub struct EddsaSignature<E: JubjubEngine> {
    pub r: EdwardsPoint<E>,
    pub s: AllocatedNum<E>,
    pub pk: EdwardsPoint<E>
}

use ::alt_babyjubjub::{fs::Fs};

impl <E: JubjubEngine>EddsaSignature<E> {

    pub fn verify_on_message<CS>(
        &self,
        mut cs: CS,
        params: &E::Params,
        personalization: Personalization,
        message: &[Boolean],
        generator: EdwardsPoint<E>
    ) -> Result<Boolean, SynthesisError>
        where CS: ConstraintSystem<E>
    {
        // TODO check that s < Fs::Char
        let scalar_bits = field_into_boolean_vec_le(cs.namespace(|| "Get S bits"), self.s.get_value());
        assert!(scalar_bits.is_ok());
        let scalar_bits_conv: &[Boolean] = &(scalar_bits.unwrap());
        let sb = generator.mul(cs.namespace(|| "S*B computation"), &scalar_bits_conv, params).unwrap();

        let personalization_bytes: &[u8] = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        // h = Hash(R, pubkey, message)

        // only order of R is checked. Public key and generator can be guaranteed to be in proper group!
        // by some other means for out particular case
        let r_is_in_order = self.r.assert_not_small_order(cs.namespace(|| "R is in right order"), &params);

        assert!(r_is_in_order.is_ok());
        // let bit_length = scalar_bits_conv.len();
        // let zero_bit = Boolean::Constant(false);

        let mut hash_bits: Vec<Boolean> = vec![];

        let r_x_value = self.r.get_x().get_value();
        let r_x_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize R_X"), r_x_value).unwrap();
        print!("{}\n", r_x_serialized.len());

        let mut to_append = 256 - r_x_serialized.len();
        for _ in 0..to_append {
            hash_bits.push(Boolean::Constant(false));
        }
        hash_bits.extend(r_x_serialized.into_iter());

        let r_y_value = self.r.get_y().get_value();
        let r_y_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize R_Y"), r_y_value).unwrap();
        to_append = 256 - r_y_serialized.len();
        for _ in 0..to_append {
            hash_bits.push(Boolean::Constant(false));
        }
        hash_bits.extend(r_y_serialized.into_iter());

        let pk_x_value = self.pk.get_x().get_value();
        let pk_x_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize PK_X"), pk_x_value).unwrap();
        to_append = 256 - pk_x_serialized.len();
        for _ in 0..to_append {
            hash_bits.push(Boolean::Constant(false));
        }
        hash_bits.extend(pk_x_serialized.into_iter());

        let pk_y_value = self.pk.get_y().get_value();
        let pk_y_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize PK_Y"), pk_y_value).unwrap();
        to_append = 256 - pk_y_serialized.len();
        for _ in 0..to_append {
            hash_bits.push(Boolean::Constant(false));
        }
        hash_bits.extend(pk_y_serialized.into_iter());
        // for i in 0..message.len() {
        //     let bit = message[i];
        //     hash_bits.push(bit);
        // }
        hash_bits.extend(message.iter().cloned());

        let hash_content_slice: &[Boolean] = &hash_bits;
        assert!(hash_content_slice.len() == 256);

        print!("{}\n", hash_content_slice.len());

        for e in hash_content_slice.into_iter() {
            if e.get_value().unwrap() {
                print!("{}", 1);
            } else {
                print!("{}", 0);
            }
        }

        let h = blake2s(cs.namespace(|| "Calculate EdDSA hash"), hash_content_slice, &personalization_bytes).unwrap();
        
        let pk_mul_hash = self.pk.mul(cs.namespace(|| "Calculate h*PK"), &h, params).unwrap();

        let rhs = pk_mul_hash.add(cs.namespace(|| "Make signature RHS"), &self.r, params).unwrap();

        let rhs_x = rhs.get_x();
        let rhs_y = rhs.get_y();

        let sb_x = sb.get_x();
        let sb_y = sb.get_y();

        // let one = CS::one();
        // cs.enforce(
        //     || "check x coordinate of signature",
        //     |lc| lc + rhs_x.get_variable(),
        //     |lc| lc + one,
        //     |lc| lc + sb_x.get_variable()
        // );

        // cs.enforce(
        //     || "check y coordinate of signature",
        //     |lc| lc + rhs_y.get_variable(),
        //     |lc| lc + one,
        //     |lc| lc + sb_y.get_variable()
        // );

        return Ok(Boolean::constant(true));
    }

    pub fn verify_eddsa_for_snark<CS>(
        &self,
        mut cs: CS,
        params: &E::Params,
        personalization: Personalization,
        message: &[Boolean],
        generator: EdwardsPoint<E>
    ) -> Result<Boolean, SynthesisError>
        where CS: ConstraintSystem<E>
    {
        // TODO check that s < Fs::Char
        let scalar_bits = field_into_boolean_vec_le(cs.namespace(|| "Get S bits"), self.s.get_value());
        assert!(scalar_bits.is_ok());
        let scalar_bits_conv: &[Boolean] = &(scalar_bits.unwrap());
        let sb = generator.mul(cs.namespace(|| "S*B computation"), &scalar_bits_conv, params).unwrap();

        let personalization_bytes: &[u8] = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        // h = Hash(R_X || message)

        // only order of R is checked. Public key and generator can be guaranteed to be in proper group!
        // by some other means for out particular case
        let r_is_in_order = self.r.assert_not_small_order(cs.namespace(|| "R is in right order"), &params);

        assert!(r_is_in_order.is_ok());

        let mut hash_bits: Vec<Boolean> = vec![];

        let r_x_value = self.r.get_x().get_value();
        let r_x_serialized = field_into_boolean_vec_le(cs.namespace(|| "Serialize R_X"), r_x_value).unwrap();

        let mut to_append = 256 - r_x_serialized.len();
        hash_bits.extend(r_x_serialized.into_iter());
        for _ in 0..to_append {
            hash_bits.push(Boolean::Constant(false));
        }

        to_append = 256 - message.len();
        hash_bits.extend(message.iter().cloned());
        for _ in 0..to_append {
            hash_bits.push(Boolean::Constant(false));
        }

        let hash_content_slice: &[Boolean] = &hash_bits;
        assert!(hash_content_slice.len() == 512);

        let h = blake2s(cs.namespace(|| "Calculate EdDSA hash"), hash_content_slice, &personalization_bytes).unwrap();
        
        let pk_mul_hash = self.pk.mul(cs.namespace(|| "Calculate h*PK"), &h, params).unwrap();

        let rhs = pk_mul_hash.add(cs.namespace(|| "Make signature RHS"), &self.r, params).unwrap();

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

        return Ok(Boolean::constant(true));
    }

    pub fn verify_raw_message_signature<CS>(
        &self,
        mut cs: CS,
        params: &E::Params,
        message: &[Boolean],
        generator: EdwardsPoint<E>,
        max_message_len: usize
    ) -> Result<Boolean, SynthesisError>
        where CS: ConstraintSystem<E>
    {
        // TODO check that s < Fs::Char

        // message is always padded to 256 bits in this gadget, but still checked on synthesis
        assert!(message.len() <= max_message_len * 8);

        let scalar_bits = field_into_boolean_vec_le(cs.namespace(|| "Get S bits"), self.s.get_value());
        assert!(scalar_bits.is_ok());
        let scalar_bits_conv: &[Boolean] = &(scalar_bits.unwrap());
        let sb = generator.mul(cs.namespace(|| "S*B computation"), &scalar_bits_conv, params).unwrap();

        // only order of R is checked. Public key and generator can be guaranteed to be in proper group!
        // by some other means for out particular case
        let r_is_in_order = self.r.assert_not_small_order(cs.namespace(|| "R is in right order"), &params);

        assert!(r_is_in_order.is_ok());

        let mut h: Vec<Boolean> = vec![];
        let to_append = 256 - message.len();

        h.extend(message.iter().cloned());
        for _ in 0..to_append {
            h.push(Boolean::Constant(false));
        }
        assert!(h.len() == 256);
        
        let pk_mul_hash = self.pk.mul(cs.namespace(|| "Calculate h*PK"), &h, params).unwrap();

        let rhs = pk_mul_hash.add(cs.namespace(|| "Make signature RHS"), &self.r, params).unwrap();

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

        return Ok(Boolean::constant(true));
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
    use ::alt_babyjubjub::fs::Fs;
    
    #[test]
    fn test_valid_for_snark_signatures() {
        
        let mut rng = XorShiftRng::from_seed([0x3dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let params = &AltJubjubBn256::new();
        let personalization = Personalization::NoteCommitment;
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
        signature.verify_eddsa_for_snark(cs.namespace(|| "verify signature"), params, personalization, &input_bools, generator).expect("succesfully generated verifying gadget");

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


