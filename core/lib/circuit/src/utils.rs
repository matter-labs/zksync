// External deps
use zksync_crypto::franklin_crypto::{
    bellman::{
        pairing::{
            ff::{BitIterator, Field, PrimeField},
            Engine,
        },
        ConstraintSystem, SynthesisError,
    },
    circuit::{
        boolean::{AllocatedBit, Boolean},
        multipack,
        num::{AllocatedNum, Num},
        Assignment,
    },
    eddsa::{PrivateKey, PublicKey, Seed, Signature},
    jubjub::{FixedGenerators, JubjubEngine},
    rescue::{rescue_hash, RescueEngine},
};
// Workspace deps
use zksync_crypto::{
    circuit::utils::le_bit_vector_into_field_element, params as franklin_constants, primitives::*,
};
// Local deps
use crate::operation::{SignatureData, TransactionSignature};

pub fn reverse_bytes<T: Clone>(bits: &[T]) -> Vec<T> {
    bits.chunks(8)
        .rev()
        .map(|x| x.to_vec())
        .fold(Vec::new(), |mut acc, mut byte| {
            acc.append(&mut byte);
            acc
        })
}

pub fn sign_sha256<E>(
    msg_data: &[bool],
    private_key: &PrivateKey<E>,
    p_g: FixedGenerators,
    params: &E::Params,
) -> SignatureData
where
    E: JubjubEngine,
{
    let message_bytes = BitConvert::into_bytes(msg_data.to_vec());

    let seed = Seed::deterministic_seed(&private_key, &message_bytes);
    let signature = private_key.musig_sha256_sign(&message_bytes, &seed, p_g, params);

    let pk = PublicKey::from_private(&private_key, p_g, params);
    let _is_valid_signature = pk.verify_musig_sha256(&message_bytes, &signature, p_g, params);

    // TODO: handle the case where it is not valid
    // if !is_valid_signature {
    //     return None;
    // }
    let (sig_r_x, sig_r_y) = signature.r.into_xy();
    log::debug!("signature.s: {}", signature.s);
    log::debug!("signature.r.x: {}", sig_r_x);
    log::debug!("signature.r.y: {}", sig_r_y);

    convert_signature_to_representation(signature)
}

pub fn sign_rescue<E>(
    msg_data: &[bool],
    private_key: &PrivateKey<E>,
    p_g: FixedGenerators,
    rescue_params: &<E as RescueEngine>::Params,
    jubjub_params: &<E as JubjubEngine>::Params,
) -> SignatureData
where
    E: RescueEngine + JubjubEngine,
{
    let message_bytes = BitConvert::into_bytes(msg_data.to_vec());

    let seed = Seed::deterministic_seed(&private_key, &message_bytes);
    let signature =
        private_key.musig_rescue_sign(&message_bytes, &seed, p_g, rescue_params, jubjub_params);

    let pk = PublicKey::from_private(&private_key, p_g, jubjub_params);
    let _is_valid_signature = pk.verify_musig_rescue(
        &message_bytes,
        &signature,
        p_g,
        rescue_params,
        jubjub_params,
    );

    // TODO: handle the case where it is not valid
    // if !is_valid_signature {
    //     return None;
    // }
    let (sig_r_x, sig_r_y) = signature.r.into_xy();
    log::debug!("signature.s: {}", signature.s);
    log::debug!("signature.r.x: {}", sig_r_x);
    log::debug!("signature.r.y: {}", sig_r_y);

    convert_signature_to_representation(signature)
}

pub fn convert_signature_to_representation<E>(signature: Signature<E>) -> SignatureData
where
    E: JubjubEngine,
{
    let (sig_x, sig_y) = signature.r.into_xy();
    let mut signature_s_be_bits: Vec<bool> = BitIterator::new(signature.s.into_repr()).collect();
    signature_s_be_bits.reverse();
    resize_grow_only(
        &mut signature_s_be_bits,
        franklin_constants::FR_BIT_WIDTH_PADDED,
        false,
    );
    signature_s_be_bits.reverse();
    let mut signature_r_x_be_bits: Vec<bool> = BitIterator::new(sig_x.into_repr()).collect();
    signature_r_x_be_bits.reverse();
    resize_grow_only(
        &mut signature_r_x_be_bits,
        franklin_constants::FR_BIT_WIDTH_PADDED,
        false,
    );
    signature_r_x_be_bits.reverse();
    let mut signature_r_y_be_bits: Vec<bool> = BitIterator::new(sig_y.into_repr()).collect();
    signature_r_y_be_bits.reverse();
    resize_grow_only(
        &mut signature_r_y_be_bits,
        franklin_constants::FR_BIT_WIDTH_PADDED,
        false,
    );
    signature_r_y_be_bits.reverse();
    let mut sig_r_packed_bits = vec![];
    sig_r_packed_bits.push(signature_r_x_be_bits[franklin_constants::FR_BIT_WIDTH_PADDED - 1]);
    sig_r_packed_bits.extend(signature_r_y_be_bits[1..].iter());
    let sig_r_packed_bits = reverse_bytes(&sig_r_packed_bits);

    assert_eq!(
        sig_r_packed_bits.len(),
        franklin_constants::FR_BIT_WIDTH_PADDED
    );

    let sig_s_bits = signature_s_be_bits;
    let sig_s_bits = reverse_bytes(&sig_s_bits);

    SignatureData {
        r_packed: sig_r_packed_bits.iter().map(|x| Some(*x)).collect(),
        s: sig_s_bits.iter().map(|x| Some(*x)).collect(),
    }
}

pub fn sign_sha<E>(
    msg_data: &[bool],
    private_key: &PrivateKey<E>,
    p_g: FixedGenerators,
    params: &E::Params,
) -> Option<TransactionSignature<E>>
where
    E: JubjubEngine,
{
    let raw_data: Vec<bool> = msg_data.to_vec();

    let mut message_bytes: Vec<u8> = vec![];

    let byte_chunks = raw_data.chunks(8);
    for byte_chunk in byte_chunks {
        let mut byte = 0u8;
        for (i, bit) in byte_chunk.iter().enumerate() {
            if *bit {
                byte |= 1 << (7 - i); //TODO: ask shamatar why do we need rev here, but not in pedersen
            }
        }
        message_bytes.push(byte);
    }

    let seed = Seed::deterministic_seed(&private_key, &message_bytes);
    let signature = private_key.musig_sha256_sign(&message_bytes, &seed, p_g, params);

    let pk = PublicKey::from_private(&private_key, p_g, params);
    let is_valid_signature = pk.verify_musig_sha256(&message_bytes, &signature, p_g, params);
    if !is_valid_signature {
        return None;
    }

    let mut sigs_le_bits: Vec<bool> = BitIterator::new(signature.s.into_repr()).collect();
    sigs_le_bits.reverse();

    let sigs_converted = le_bit_vector_into_field_element(&sigs_le_bits);

    Some(TransactionSignature {
        r: signature.r,
        s: sigs_converted,
    })
}

pub fn multi_and<E: Engine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    x: &[Boolean],
) -> Result<Boolean, SynthesisError> {
    let mut result = Boolean::constant(true);

    for (i, bool_x) in x.iter().enumerate() {
        result = Boolean::and(
            cs.namespace(|| format!("multi and iteration number: {}", i)),
            &result,
            bool_x,
        )?;
    }

    Ok(result)
}

pub fn allocate_sum<E: Engine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    a: &AllocatedNum<E>,
    b: &AllocatedNum<E>,
) -> Result<AllocatedNum<E>, SynthesisError> {
    let sum = AllocatedNum::alloc(cs.namespace(|| "sum"), || {
        let mut sum = a.get_value().grab()?;
        sum.add_assign(&b.get_value().grab()?);
        Ok(sum)
    })?;
    cs.enforce(
        || "enforce sum",
        |lc| lc + a.get_variable() + b.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + sum.get_variable(),
    );

    Ok(sum)
}

pub fn pack_bits_to_element<E: Engine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    bits: &[Boolean],
) -> Result<AllocatedNum<E>, SynthesisError> {
    assert!(
        bits.len() <= E::Fr::NUM_BITS as usize,
        "can not pack bits into field element: number of bits is larger than number of bits in a field"
    );
    let mut data_from_lc = Num::<E>::zero();
    let mut coeff = E::Fr::one();
    for bit in bits {
        data_from_lc = data_from_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let data_packed = AllocatedNum::alloc(cs.namespace(|| "allocate account data packed"), || {
        Ok(*data_from_lc.get_value().get()?)
    })?;

    cs.enforce(
        || "pack account data",
        |lc| lc + data_packed.get_variable(),
        |lc| lc + CS::one(),
        |_| data_from_lc.lc(E::Fr::one()),
    );

    Ok(data_packed)
}

pub fn pack_bits_to_element_strict<E: Engine, CS: ConstraintSystem<E>>(
    cs: CS,
    bits: &[Boolean],
) -> Result<AllocatedNum<E>, SynthesisError> {
    assert!(
        bits.len() <= E::Fr::CAPACITY as usize,
        "can not pack bits into field element over the precision"
    );

    pack_bits_to_element(cs, bits)
}

// count a number of non-zero bits in a bit decomposition
pub fn count_number_of_ones<E, CS>(
    mut cs: CS,
    a: &[Boolean],
) -> Result<AllocatedNum<E>, SynthesisError>
where
    E: Engine,
    CS: ConstraintSystem<E>,
{
    let mut counter = Num::zero();
    for bit in a.iter() {
        counter = counter.add_bool_with_coeff(CS::one(), &bit, E::Fr::one());
    }

    let result = AllocatedNum::alloc(cs.namespace(|| "number of zeroes number"), || {
        Ok(*counter.get_value().get()?)
    })?;

    cs.enforce(
        || "pack number of ones",
        |lc| lc + result.get_variable(),
        |lc| lc + CS::one(),
        |_| counter.lc(E::Fr::one()),
    );

    Ok(result)
}

pub fn allocate_numbers_vec<E, CS>(
    mut cs: CS,
    witness_vec: &[Option<E::Fr>],
) -> Result<Vec<AllocatedNum<E>>, SynthesisError>
where
    E: Engine,
    CS: ConstraintSystem<E>,
{
    let mut allocated = vec![];
    for (i, e) in witness_vec.iter().enumerate() {
        let path_element =
            AllocatedNum::alloc(cs.namespace(|| format!("path element{}", i)), || {
                Ok(*e.get()?)
            })?;
        allocated.push(path_element);
    }

    Ok(allocated)
}

pub fn allocate_bits_vector<E, CS>(
    mut cs: CS,
    bits: &[Option<bool>],
) -> Result<Vec<Boolean>, SynthesisError>
where
    E: Engine,
    CS: ConstraintSystem<E>,
{
    let mut allocated = vec![];
    for (i, e) in bits.iter().enumerate() {
        let element = Boolean::from(AllocatedBit::alloc(
            cs.namespace(|| format!("path element{}", i)),
            *e,
        )?);
        allocated.push(element);
    }

    Ok(allocated)
}

pub fn append_packed_public_key(
    content: &mut Vec<Boolean>,
    x_bits: Vec<Boolean>,
    y_bits: Vec<Boolean>,
) {
    assert_eq!(franklin_constants::FR_BIT_WIDTH - 1, y_bits.len());
    assert_eq!(1, x_bits.len());
    content.extend(y_bits);
    content.extend(x_bits);
}

pub fn print_boolean_vec(bits: &[Boolean]) {
    let mut bytes = vec![];
    for slice in bits.chunks(8) {
        let mut b = 0u8;
        for (i, bit) in slice.iter().enumerate() {
            if bit.get_value().unwrap() {
                b |= 1u8 << (7 - i);
            }
        }
        bytes.push(b);
    }
}

pub fn resize_grow_only<T: Clone>(to_resize: &mut Vec<T>, new_size: usize, pad_with: T) {
    assert!(to_resize.len() <= new_size);
    to_resize.resize(new_size, pad_with);
}

pub fn boolean_or<E: Engine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    x: &Boolean,
    y: &Boolean,
) -> Result<Boolean, SynthesisError> {
    // A OR B = ( A NAND A ) NAND ( B NAND B ) = (NOT(A)) NAND (NOT (B))
    let result = Boolean::and(
        cs.namespace(|| "lhs_valid nand rhs_valid"),
        &x.not(),
        &y.not(),
    )?
    .not();

    Ok(result)
}

pub fn calculate_empty_balance_tree_hashes<E: RescueEngine>(
    rescue_params: &E::Params,
    tree_depth: usize,
) -> Vec<E::Fr> {
    let empty_balance = E::Fr::zero();
    calculate_empty_tree_hashes::<E>(rescue_params, tree_depth, &[empty_balance])
}

pub fn calculate_empty_account_tree_hashes<E: RescueEngine>(
    rescue_params: &E::Params,
    tree_depth: usize,
) -> Vec<E::Fr> {
    // manually calcualte empty subtree hashes
    let empty_account_packed =
        zksync_crypto::circuit::account::empty_account_as_field_elements::<E>();
    calculate_empty_tree_hashes::<E>(rescue_params, tree_depth, &empty_account_packed)
}

fn calculate_empty_tree_hashes<E: RescueEngine>(
    rescue_params: &E::Params,
    tree_depth: usize,
    packed_leaf: &[E::Fr],
) -> Vec<E::Fr> {
    let empty_leaf_hash = {
        let mut sponge_output = rescue_hash::<E>(rescue_params, packed_leaf);
        assert_eq!(sponge_output.len(), 1);
        sponge_output.pop().unwrap()
    };

    let mut current = empty_leaf_hash;
    let mut empty_node_hashes = vec![];
    for _ in 0..tree_depth {
        let node_hash = {
            let mut sponge_output = rescue_hash::<E>(rescue_params, &[current, current]);
            assert_eq!(sponge_output.len(), 1);
            sponge_output.pop().unwrap()
        };
        empty_node_hashes.push(node_hash);
        current = node_hash;
    }
    empty_node_hashes
}

pub fn vectorized_compare<E: Engine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    old_data: &[AllocatedNum<E>],
    new_bits: &[Boolean],
) -> Result<(Boolean, Vec<AllocatedNum<E>>), SynthesisError> {
    let packed = multipack::pack_into_witness(cs.namespace(|| "pack claimed data"), &new_bits)?;

    assert_eq!(packed.len(), old_data.len());

    // compare

    let mut equality_bits = vec![];

    for (i, (old, new)) in old_data.iter().zip(packed.iter()).enumerate() {
        let is_equal_bit = AllocatedNum::<E>::equals(
            cs.namespace(|| format!("equality for chunk {}", i)),
            &old,
            &new,
        )?;

        let equal_bool = Boolean::from(is_equal_bit);
        equality_bits.push(equal_bool);
    }

    let is_equal = multi_and(cs.namespace(|| "all data is equal"), &equality_bits)?;

    Ok((is_equal, packed))
}
