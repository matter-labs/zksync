use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{PrimeField, PrimeFieldRepr};
use pairing::{Engine};
use pairing::bn256::{Bn256, Fr};

use sapling_crypto::circuit::sha256::{sha256, sha256_block_no_padding};
use sapling_crypto::circuit::num::{AllocatedNum};
use sapling_crypto::circuit::boolean::{Boolean, AllocatedBit};
use sapling_crypto::circuit::multipack;
use sapling_crypto::circuit::multipack::{pack_into_inputs};
use crypto::sha2::Sha256;
use crypto::digest::Digest;

struct HardcodedSha256Circuit<E: Engine> {
    preimage: Option<E::Fr>,
}

fn print_boolean_vector(vector: &[Boolean]) {
    for b in vector {
        if b.get_value().unwrap() {
            print!("1");
        } else {
            print!("0");
        }
    }
    print!("\n");
}

// fn change_endianess(initial: &Vec<Boolean>) -> Vec<Boolean> {
//     let mut result: Vec<Boolean> = vec![];
//     for chunk in initial.chunks(8) {
//         let reversed: Vec<Boolean> = chunk.into_iter().rev().collect();
//         result.extend(&reversed.into_iter());
//         // for e in chunk.into_iter().rev() {
//         //     result.append(e);
//         // }
//     }

//     result
// }

// Implementation of our circuit:
// Given a `hash`, prove that we know a 5 byte string `preimage` such that `sha256(preimage) == hash`
impl<E: Engine> Circuit<E> for HardcodedSha256Circuit<E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {

        // let mut preimage_bits = vec![];
        // preimage_bits.push(Boolean::Constant(true));
        // for _ in 0..511 {
        //     preimage_bits.push(Boolean::Constant(false));
        // }

        let mut preimage_bits = vec![];
        for _ in 0..7 {
            preimage_bits.push(Boolean::Constant(false));
        }
        preimage_bits.push(Boolean::Constant(true));
        for _ in 0..504 {
            preimage_bits.push(Boolean::Constant(false));
        }

        let mut preimage_reversed = preimage_bits.clone();
        preimage_reversed.reverse();

        // get sha256 bits
        let mut hash_bits = sha256(cs.namespace(|| "sha256"), &preimage_bits).unwrap();

        let mut hash_bits_from_rev = sha256(cs.namespace(|| "sha256 from rev"), &preimage_reversed).unwrap();

        print_boolean_vector(&hash_bits.clone());
        // hash_bits.reverse();
        // print_boolean_vector(&hash_bits.clone());

        // print_boolean_vector(&hash_bits_from_rev.clone());
        // hash_bits_from_rev.reverse();
        // print_boolean_vector(&hash_bits_from_rev.clone());

        hash_bits.truncate(E::Fr::CAPACITY as usize);

        // allocate hash bits as an input field element
        pack_into_inputs(cs.namespace(|| "hash"), hash_bits.as_slice()).unwrap();

        Ok(())
    }
}

#[test]
fn test_sha256_bitness() {
    use ff::{Field};
    use pairing::bn256::*;
    use rand::{SeedableRng, Rng, XorShiftRng, Rand};
    use sapling_crypto::circuit::test::*;
    use sapling_crypto::alt_babyjubjub::{AltJubjubBn256, fs, edwards, PrimeOrder};
    use balance_tree::{BabyBalanceTree, BabyLeaf, Leaf};
    use crypto::sha2::Sha256;
    use crypto::digest::Digest;
    use sapling_crypto::circuit::test::*;

    use rand::thread_rng;

    let mut cs = TestConstraintSystem::<Bn256>::new();

    let rng = &mut thread_rng();

    let instance = HardcodedSha256Circuit::<Bn256> {
        preimage: None,
    };

    instance.synthesize(&mut cs).unwrap();

    let mut data_to_hash = [0u8; 64];
    data_to_hash[0] |= 0x01;

    let mut h = Sha256::new();
    h.input(&data_to_hash);

    let mut hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    let mut first_round_bits = multipack::bytes_to_bits(&hash_result.clone());
            
    for b in first_round_bits.clone() {
        if b {
            print!("1");
        } else {
            print!("0");
        }
    }
    print!("\n");

    

    // let mut first_round_bits_le = multipack::bytes_to_bits_le(&hash_result);
    
    // for b in first_round_bits_le.clone() {
    //     if b {
    //         print!("1");
    //     } else {
    //         print!("0");
    //     }
    // }
    // print!("\n");

    // data_to_hash = [0u8; 64];

    // data_to_hash[0] |= 0x80;

    // h = Sha256::new();
    // assert_eq!(h.block_size(), 64);

    // h.input(&data_to_hash);

    // hash_result = [0u8; 32];
    // h.result(&mut hash_result[..]);

    // first_round_bits = multipack::bytes_to_bits(&hash_result.clone());
            
    // for b in first_round_bits.clone() {
    //     if b {
    //         print!("1");
    //     } else {
    //         print!("0");
    //     }
    // }
    // print!("\n");

    // first_round_bits_le = multipack::bytes_to_bits_le(&hash_result);
    
    // for b in first_round_bits_le.clone() {
    //     if b {
    //         print!("1");
    //     } else {
    //         print!("0");
    //     }
    // }
    // print!("\n");
}

#[test]
    fn test_against_vectors() {
        use crypto::sha2::Sha256;
        use crypto::digest::Digest;
        use rand::{SeedableRng, Rng, XorShiftRng, Rand};
        use sapling_crypto::circuit::test::*;
        let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

        let input_len = 64;
        {
            let mut h = Sha256::new();
            // let data: Vec<u8> = (0..input_len).map(|_| rng.gen()).collect();
            let mut data = [0u8; 64];
            data[0] = 0x01;
            h.input(&data);
            let mut hash_result = [0u8; 32];
            h.result(&mut hash_result[..]);

            let mut cs = TestConstraintSystem::<Bn256>::new();
            let mut input_bits = vec![];

            // this is BE encoding

            for (byte_i, input_byte) in data.into_iter().enumerate() {
                for bit_i in (0..8).rev() {
                    let cs = cs.namespace(|| format!("input bit {} {}", byte_i, bit_i));

                    let boolean_value = (input_byte >> bit_i) & 1u8 == 1u8;
                    if boolean_value {
                        print!("1");
                    } else {
                        print!("0");
                    }

                    input_bits.push(AllocatedBit::alloc(cs, Some(boolean_value)).unwrap().into());
                }
            }

            let r = sha256(&mut cs, &input_bits).unwrap();

            // let r = sha256_block_no_padding(&mut cs, &input_bits).unwrap();

            assert!(cs.is_satisfied());

            let mut s = hash_result.as_ref().iter()
                                            .flat_map(|&byte| (0..8).rev().map(move |i| (byte >> i) & 1u8 == 1u8));

            for b in r {
                match b {
                    Boolean::Is(b) => {
                        assert!(s.next().unwrap() == b.get_value().unwrap());
                    },
                    Boolean::Not(b) => {
                        assert!(s.next().unwrap() != b.get_value().unwrap());
                    },
                    Boolean::Constant(b) => {
                        assert!(input_len == 0);
                        assert!(s.next().unwrap() == b);
                    }
                }
            }
        }
    }