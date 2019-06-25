use crate::franklin::franklin_constants;
use crate::franklin::utils::append_packed_public_key;
use franklin_crypto::jubjub::JubjubEngine;

use bellman::{ConstraintSystem, SynthesisError};
use franklin_crypto::circuit::boolean;
use franklin_crypto::circuit::num::AllocatedNum;
use franklin_crypto::circuit::Assignment;

#[derive(Clone, Debug)]
pub struct AccountWitness<E: JubjubEngine> {
    pub nonce: Option<E::Fr>,
    // x coordinate is supplied and parity is constrained
    pub pub_x: Option<E::Fr>,
    pub pub_y: Option<E::Fr>,
}

pub struct AccountContent<E: JubjubEngine> {
    pub bits: AccountContentBitForm,
    pub base: AccountContentBase<E>, //TODO: suggest more appropriate naming
}

pub struct AccountContentBase<E: JubjubEngine> {
    // pub leaf_bits: Vec<boolean::Boolean>,
    // pub subtree_merkle_root_bits: Vec<boolean::Boolean>,
    pub nonce: AllocatedNum<E>,
    // pub nonce_bits: Vec<boolean::Boolean>,
    pub pub_x: AllocatedNum<E>,
    pub pub_y: AllocatedNum<E>,
    // pub pub_x_bit: Vec<boolean::Boolean>,
    // pub pub_y_bits: Vec<boolean::Boolean>,
}
impl<E: JubjubEngine> AccountContentBase<E> {
    pub fn make_bit_form<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
    ) -> Result<AccountContentBitForm, SynthesisError> {
        let mut nonce_bits = self.nonce.into_bits_le(cs.namespace(|| "nonce bits"))?;

        nonce_bits.truncate(*franklin_constants::NONCE_BIT_WIDTH);

        let mut pub_x_bit = self.pub_x.into_bits_le(cs.namespace(|| "pub_x bits"))?;
        // leave only the parity bit
        pub_x_bit.truncate(1);

        let mut pub_y_bits = self.pub_y.into_bits_le(cs.namespace(|| "pub_y bits"))?;
        pub_y_bits.resize(
            franklin_constants::FR_BIT_WIDTH - 1,
            boolean::Boolean::Constant(false),
        );

        // append_packed_public_key(&mut leaf_bits, pub_x_bit.clone(), pub_y_bits.clone());

        // assert_eq!(leaf_bits.len(), franklin_constants::SUBTREE_HASH_WIDTH
        //                             + franklin_constants::NONCE_BIT_WIDTH
        //                             + franklin_constants::FR_BIT_WIDTH
        // );
        Ok(AccountContentBitForm {
            nonce_bits: nonce_bits,
            pub_x_bit: pub_x_bit,
            pub_y_bits: pub_y_bits,
        })
    }
}
pub struct AccountContentBitForm {
    pub nonce_bits: Vec<boolean::Boolean>,
    pub pub_x_bit: Vec<boolean::Boolean>,
    pub pub_y_bits: Vec<boolean::Boolean>,
}
pub fn make_account_content<E, CS>(
    mut cs: CS,
    witness: &AccountWitness<E>,
) -> Result<AccountContent<E>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    let account_content_base =
        make_account_content_base(cs.namespace(|| "allocating account_content_base"), &witness)?;
    let account_content_bits = account_content_base.make_bit_form(cs.namespace(|| "bits_form"))?;
    Ok(AccountContent {
        base: account_content_base,
        bits: account_content_bits,
    })
}

pub fn make_account_content_base<E, CS>(
    mut cs: CS,
    witness: &AccountWitness<E>,
) -> Result<AccountContentBase<E>, SynthesisError>
where
    E: JubjubEngine,
    CS: ConstraintSystem<E>,
{
    let nonce = AllocatedNum::alloc(cs.namespace(|| "allocate leaf nonce witness"), || {
        Ok(*witness.nonce.get()?)
    })?;

    // we allocate (witness) public X and Y to use them also later for signature check

    let pub_x = AllocatedNum::alloc(cs.namespace(|| "allocate public key x witness"), || {
        Ok(*witness.pub_x.get()?)
    })?;

    let pub_y = AllocatedNum::alloc(cs.namespace(|| "allocate public key y witness"), || {
        Ok(*witness.pub_y.get()?)
    })?;

    let mut pub_y_bits = pub_y.into_bits_le(cs.namespace(|| "pub_y bits"))?;

    Ok(AccountContentBase {
        nonce: nonce,
        pub_x: pub_x,
        pub_y: pub_y,
    })
}
