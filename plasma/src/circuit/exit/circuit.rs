use ff::{
    PrimeField,
    Field,
    BitIterator,
    PrimeFieldRepr
};

use bellman::{
    SynthesisError,
    ConstraintSystem,
    Circuit
};

use sapling_crypto::jubjub::{
    JubjubEngine,
    FixedGenerators,
    Unknown,
    edwards,
    JubjubParams
};

use super::Assignment;
use super::boolean;
use super::ecc;
use super::pedersen_hash;
use super::sha256;
use super::num;
use super::multipack;
use super::num::{AllocatedNum, Num};
use super::float_point::{parse_with_exponent_le, convert_to_float};
use super::baby_eddsa::EddsaSignature;

use sapling_crypto::eddsa::{
    Signature,
    PrivateKey,
    PublicKey
};

<<<<<<< HEAD
use crate::models::params;
use crate::circuit::utils::le_bit_vector_into_field_element;
=======
use crate::circuit::plasma_constants;
use super::super::leaf::{LeafWitness, LeafContent, make_leaf_content};
use crate::circuit::utils::{le_bit_vector_into_field_element, allocate_audit_path, append_packed_public_key};
>>>>>>> more_ff
use super::exit_request::{ExitRequest};

#[derive(Clone)]
pub struct ExitWitness<E: JubjubEngine> {
    pub leaf: LeafWitness<E>,
    pub auth_path: Vec<Option<E::Fr>>,
}

/// This is an instance of the `Spend` circuit.
pub struct Exit<'a, E: JubjubEngine> {
    pub params: &'a E::Params,

    // number of exits per block
    pub number_of_exits: usize,

    /// The old root of the tree
    pub old_root: Option<E::Fr>,

    /// The new root of the tree
    pub new_root: Option<E::Fr>,

    /// Final truncated rolling SHA256
    pub public_data_commitment: Option<E::Fr>,

    /// Supply witness for an empty leaf once
    pub empty_leaf_witness: LeafWitness<E>,

    /// Block number
    pub block_number: Option<E::Fr>,

    /// Requests for this block
    pub requests: Vec<(ExitRequest<E>, ExitWitness<E>)>,
}


// for now there is no check in this gadget that exit is done from leaf with 
// non-zero public key. It's intended on application level for exited leafs to be non-empty

impl<'a, E: JubjubEngine> Circuit<E> for Exit<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError>
    {
        // Check that transactions are in a right quantity
        assert!(self.number_of_exits == self.requests.len());

        let old_root_value = self.old_root;
        // Expose inputs and do the bits decomposition of hash
        let mut old_root = AllocatedNum::alloc(
            cs.namespace(|| "old root"),
            || Ok(*old_root_value.get()?)
        )?;
        old_root.inputize(cs.namespace(|| "old root input"))?;

        let new_root_value = self.new_root;
        let new_root = AllocatedNum::alloc(
            cs.namespace(|| "new root"),
            || Ok(*new_root_value.get()?)
        )?;
        new_root.inputize(cs.namespace(|| "new root input"))?;

        let rolling_hash_value = self.public_data_commitment;
        let rolling_hash = AllocatedNum::alloc(
            cs.namespace(|| "rolling hash"),
            || Ok(*rolling_hash_value.get()?)
        )?;
        rolling_hash.inputize(cs.namespace(|| "rolling hash input"))?;

        let mut public_data_vector: Vec<boolean::Boolean> = vec![];

        // allocate empty leaf witness and make hash out of it

        // Calculate leaf value commitment

        let empty_leaf = make_leaf_content(
            cs.namespace(|| "create leaf"),
            self.empty_leaf_witness.clone()
        )?;

        // constraint empty balance, nonce, pub_x and pub_y

        cs.enforce(
            || "boolean constraint for balance is zero for empty leaf",
            |lc| lc + empty_leaf.value.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc
        );

        cs.enforce(
            || "boolean constraint for nonce is zero for empty leaf",
            |lc| lc + empty_leaf.nonce.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc
        );

        cs.enforce(
            || "boolean constraint for pub_x is zero for empty leaf",
            |lc| lc + empty_leaf.pub_x.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc
        );

        cs.enforce(
            || "boolean constraint for pub_y is zero for empty leaf",
            |lc| lc + empty_leaf.pub_y.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc
        );

        // Compute the hash of the from leaf
        let empty_leaf_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "leaf content hash"),
            pedersen_hash::Personalization::NoteCommitment,
            &empty_leaf.leaf_bits,
            self.params
        )?;

        // Ok, now we need to update the old root by applying requests in sequence
        let requests = self.requests.clone();

        let empty_leaf_x = empty_leaf_hash.get_x();

        for (i, tx) in requests.into_iter().enumerate() {
            let (request, witness) = tx;
            let (intermediate_root, public_data) = apply_request(
                cs.namespace(|| format!("applying transaction {}", i)),
                old_root,
                &empty_leaf_x,
                request,
                witness,
                self.params,
            )?;
            old_root = intermediate_root;
            // flatten the public transaction data
            public_data_vector.extend(public_data.into_iter());
        }

        // constraint the new hash to be equal to updated hash

        cs.enforce(
            || "enforce new root equal to recalculated one",
            |lc| lc + new_root.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + old_root.get_variable()
        );

        // Inside the circuit with work with LE bit order, 
        // so an account number "1" that would have a natural representation of e.g. 0x000001
        // will have a bit decomposition [1, 0, 0, 0, ......]

        // Don't deal with it here, but rather do on application layer when parsing the data!
        // The only requirement is to properly seed initial hash value with block number and fees,
        // as those are going to be naturally represented as Ethereum units

        // Now it's time to pack the initial SHA256 hash due to Ethereum BE encoding
        // and start rolling the hash

        let mut initial_hash_data: Vec<boolean::Boolean> = vec![];

        let block_number_allocated = AllocatedNum::alloc(
            cs.namespace(|| "allocate block number"),
            || {
                Ok(*self.block_number.get()?)
            }
        )?;

        // make initial hash as sha256(uint256(block_number))
        let mut block_number_bits = block_number_allocated.into_bits_le(
            cs.namespace(|| "unpack block number for hashing")
        )?;

        block_number_bits.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));
        block_number_bits.reverse();
        initial_hash_data.extend(block_number_bits.into_iter());

        assert_eq!(initial_hash_data.len(), 256);

        let mut hash_block = sha256::sha256(
            cs.namespace(|| "initial rolling sha256"),
            &initial_hash_data
        )?;

        // now pack the public data and do the final hash

        let mut pack_bits = vec![];
        pack_bits.extend(hash_block);
        pack_bits.extend(public_data_vector.into_iter());

        hash_block = sha256::sha256(
            cs.namespace(|| "hash public data"),
            &pack_bits
        )?;

        // // now pack and enforce equality to the input

        hash_block.reverse();
        hash_block.truncate(E::Fr::CAPACITY as usize);

        let mut packed_hash_lc = Num::<E>::zero();
        let mut coeff = E::Fr::one();
        for bit in hash_block {
            packed_hash_lc = packed_hash_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
            coeff.double();
        }

        cs.enforce(
            || "enforce external data hash equality",
            |lc| lc + rolling_hash.get_variable(),
            |lc| lc + CS::one(),
            |_| packed_hash_lc.lc(E::Fr::one())
        );

        Ok(())
    }
}

/// Applies one request to the tree,
/// outputs a new root
fn apply_request<E, CS>(
    mut cs: CS,
    old_root: AllocatedNum<E>,
    empty_leaf_x: &AllocatedNum<E>,
    request: ExitRequest<E>,
    witness: ExitWitness<E>,
    params: &E::Params
) -> Result<(AllocatedNum<E>, Vec<boolean::Boolean>), SynthesisError>
    where E: JubjubEngine,
          CS: ConstraintSystem<E>
{
    // Calculate leaf value commitment

    let leaf = make_leaf_content(
        cs.namespace(|| "create leaf"),
        witness.clone().leaf
    )?;

<<<<<<< HEAD
    balance_content_from.truncate(params::BALANCE_BIT_WIDTH);
    leaf_content.extend(balance_content_from.clone());

    let nonce_from_allocated = AllocatedNum::alloc(
        cs.namespace(|| "allocate nonce from"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.nonce.clone().get()?)
        }
    )?;

    let mut nonce_content_from = nonce_from_allocated.into_bits_le(
        cs.namespace(|| "from leaf nonce bits")
    )?;

    nonce_content_from.truncate(params::NONCE_BIT_WIDTH);
    leaf_content.extend(nonce_content_from.clone());

    // we allocate (witness) public X and Y to expose leaf content

    let sender_pk_x = AllocatedNum::alloc(
        cs.namespace(|| "sender public key x"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.pub_x.get()?)
        }
    )?;

    let sender_pk_y = AllocatedNum::alloc(
        cs.namespace(|| "sender public key y"),
        || {
            let tx_witness = &transaction.get()?.1;
            Ok(*tx_witness.pub_y.get()?)
        }
    )?;

    let mut pub_x_content_from = sender_pk_x.into_bits_le(
        cs.namespace(|| "from leaf pub_x bits")
    )?;
    pub_x_content_from.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));

    leaf_content.extend(pub_x_content_from.clone());

    let mut pub_y_content_from = sender_pk_y.into_bits_le(
        cs.namespace(|| "from leaf pub_y bits")
    )?;
    pub_y_content_from.resize(params::FR_BIT_WIDTH, boolean::Boolean::Constant(false));

    leaf_content.extend(pub_y_content_from.clone());

    assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                + params::NONCE_BIT_WIDTH
                                + 2 * (params::FR_BIT_WIDTH)
    );

=======
>>>>>>> more_ff
    // Compute the hash of the from leaf
    let mut leaf_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "leaf content hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &leaf.leaf_bits,
        params
    )?;

    // Constraint that "int" field in transaction is 
    // equal to the merkle proof path

    let address_allocated = AllocatedNum::alloc(
        cs.namespace(|| "exit from address"),
        || {
            Ok(*request.from.get()?)
        }
    )?;

<<<<<<< HEAD
    let mut from_path_bits = from_address_allocated.into_bits_le(
        cs.namespace(|| "from address bit decomposition")
    )?;

    from_path_bits.truncate(params::BALANCE_TREE_DEPTH);

    // This is an injective encoding, as cur is a
    // point in the prime order subgroup.
    let mut cur_from = from_leaf_hash.get_x().clone();

    let audit_path_from = transaction.get()?.1.clone().auth_path;
    // Ascend the merkle tree authentication path
    for (i, (e, direction_bit)) in audit_path_from.clone().into_iter().zip(from_path_bits.clone().into_iter()).enumerate() {
        let cs = &mut cs.namespace(|| format!("from merkle tree hash {}", i));

        // "direction_bit" determines if the current subtree
        // is the "right" leaf at this depth of the tree.

        // Witness the authentication path element adjacent
        // at this depth.
        let path_element = num::AllocatedNum::alloc(
            cs.namespace(|| "path element"),
            || {
                Ok(*e.get()?)
            }
        )?;

        // Swap the two if the current subtree is on the right
        let (xl, xr) = num::AllocatedNum::conditionally_reverse(
            cs.namespace(|| "conditional reversal of preimage"),
            &cur_from,
            &path_element,
            &direction_bit
        )?;

        // We don't need to be strict, because the function is
        // collision-resistant. If the prover witnesses a congruency,
        // they will be unable to find an authentication path in the
        // tree with high probability.
        let mut preimage = vec![];
        preimage.extend(xl.into_bits_le(cs.namespace(|| "xl into bits"))?);
        preimage.extend(xr.into_bits_le(cs.namespace(|| "xr into bits"))?);

        // Compute the new subtree value
        cur_from = pedersen_hash::pedersen_hash(
            cs.namespace(|| "computation of pedersen hash"),
            pedersen_hash::Personalization::MerkleTree(i),
            &preimage,
            params
        )?.get_x().clone(); // Injective encoding

    }

    // enforce old root before update
    cs.enforce(
        || "enforce correct old root for from leaf",
        |lc| lc + cur_from.get_variable(),
        |lc| lc + CS::one(),
        |lc| lc + old_root.get_variable()
    );

    // Initial leaf values are allocated, so we modify a leaf

    // reconstruct a new leaf structure
    // take all the balance and use it for exit

    // repack balances as we have truncated bit decompositions already
    let mut old_balance_from_lc = Num::<E>::zero();
    let mut coeff = E::Fr::one();
    for bit in balance_content_from.clone() {
        old_balance_from_lc = old_balance_from_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let mut nonce_lc = Num::<E>::zero();
    coeff = E::Fr::one();
    for bit in nonce_content_from.clone() {
        nonce_lc = nonce_lc.add_bool_with_coeff(CS::one(), &bit, coeff);
        coeff.double();
    }

    let old_balance_from = AllocatedNum::alloc(
        cs.namespace(|| "allocate old leaf balance"),
        || Ok(*old_balance_from_lc.get_value().get()?)
=======
    let mut path_bits = address_allocated.into_bits_le(
        cs.namespace(|| "address bit decomposition")
>>>>>>> more_ff
    )?;

    path_bits.truncate(*plasma_constants::BALANCE_TREE_DEPTH);

<<<<<<< HEAD
    let new_balance_from = AllocatedNum::alloc(
        cs.namespace(|| "new balance from"),
        || {
            Ok(E::Fr::zero())
        }
    )?;

    // constraint no overflow
    new_balance_from.limit_number_of_bits(
        cs.namespace(|| "limit number of bits for new balance from"),
        params::BALANCE_BIT_WIDTH
=======
    let audit_path = allocate_audit_path(
        cs.namespace(|| "allocate audit path"), 
        witness.clone().auth_path
>>>>>>> more_ff
    )?;

    {
        // This is an injective encoding, as cur is a
        // point in the prime order subgroup.
        let mut cur = leaf_hash.get_x().clone();

        // Ascend the merkle tree authentication path
        for (i, direction_bit) in path_bits.clone().into_iter().enumerate() {
            let cs = &mut cs.namespace(|| format!("merkle tree hash {}", i));

            // "direction_bit" determines if the current subtree
            // is the "right" leaf at this depth of the tree.

            // Witness the authentication path element adjacent
            // at this depth.
            let path_element = &audit_path[i];

            // Swap the two if the current subtree is on the right
            let (xl, xr) = num::AllocatedNum::conditionally_reverse(
                cs.namespace(|| "conditional reversal of preimage"),
                &cur,
                path_element,
                &direction_bit
            )?;

            // We don't need to be strict, because the function is
            // collision-resistant. If the prover witnesses a congruency,
            // they will be unable to find an authentication path in the
            // tree with high probability.
            let mut preimage = vec![];
            preimage.extend(xl.into_bits_le(cs.namespace(|| "xl into bits"))?);
            preimage.extend(xr.into_bits_le(cs.namespace(|| "xr into bits"))?);

            // Compute the new subtree value
            cur = pedersen_hash::pedersen_hash(
                cs.namespace(|| "computation of pedersen hash"),
                pedersen_hash::Personalization::MerkleTree(i),
                &preimage,
                params
            )?.get_x().clone(); // Injective encoding

<<<<<<< HEAD
        value_content.truncate(params::BALANCE_BIT_WIDTH);
        
        leaf_content.extend(value_content);
        leaf_content.extend(nonce_content_from);
        leaf_content.extend(pub_x_content_from.clone());
        leaf_content.extend(pub_y_content_from.clone());

        assert_eq!(leaf_content.len(), params::BALANCE_BIT_WIDTH 
                                    + params::NONCE_BIT_WIDTH
                                    + 2 * (params::FR_BIT_WIDTH));
=======
        }

        // enforce old root before update
        cs.enforce(
            || "enforce correct old root for from leaf",
            |lc| lc + cur.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + old_root.get_variable()
        );
>>>>>>> more_ff

    }

    let mut cur = empty_leaf_x.clone();

    // Ascend the merkle tree authentication path
    for (i, direction_bit) in path_bits.clone().into_iter().enumerate() {
        let cs = &mut cs.namespace(|| format!("update merkle tree hash {}", i));

        // "direction_bit" determines if the current subtree
        // is the "right" leaf at this depth of the tree.

        // Witness the authentication path element adjacent
        // at this depth.
        let path_element = &audit_path[i];

        // Swap the two if the current subtree is on the right
        let (xl, xr) = num::AllocatedNum::conditionally_reverse(
            cs.namespace(|| "conditional reversal of preimage"),
            &cur,
            path_element,
            &direction_bit
        )?;

        // We don't need to be strict, because the function is
        // collision-resistant. If the prover witnesses a congruency,
        // they will be unable to find an authentication path in the
        // tree with high probability.
        let mut preimage = vec![];
        preimage.extend(xl.into_bits_le(cs.namespace(|| "xl into bits"))?);
        preimage.extend(xr.into_bits_le(cs.namespace(|| "xr into bits"))?);

        // Compute the new subtree value
        cur = pedersen_hash::pedersen_hash(
            cs.namespace(|| "computation of pedersen hash"),
            pedersen_hash::Personalization::MerkleTree(i),
            &preimage,
            params
        )?.get_x().clone(); // Injective encoding

    }

    // the last step - we expose public data for later commitment

    // data packing should be BE
    let mut public_data = vec![];
    let mut path_bits_be = path_bits.clone();
    path_bits_be.reverse();
    public_data.extend(path_bits_be);
    let mut amount_bits_be = leaf.value_bits.clone();
    amount_bits_be.reverse();
    public_data.extend(amount_bits_be);

    assert_eq!(public_data.len(), params::BALANCE_TREE_DEPTH 
                                    + params::BALANCE_BIT_WIDTH);

    Ok((cur, public_data))
}

fn print_boolean_vector(vector: &[boolean::Boolean]) {
    for b in vector {
        if b.get_value().unwrap() {
            print!("1");
        } else {
            print!("0");
        }
    }
    print!("\n");
}

#[test]
fn test_exit_from_existing_leaf() {
    use ff::{Field, BitIterator};
    use pairing::bn256::*;
    use rand::{SeedableRng, Rng, XorShiftRng, Rand};
    use sapling_crypto::circuit::test::*;
    use sapling_crypto::alt_babyjubjub::{AltJubjubBn256, fs, edwards, PrimeOrder};
    use crate::models::circuit::{AccountTree, Account};
    use crypto::sha2::Sha256;
    use crypto::digest::Digest;
    use crate::circuit::utils::{encode_fs_into_fr, be_bit_vector_into_bytes};
    use crate::primitives::GetBits;
    extern crate hex;

    let params = &AltJubjubBn256::new();
    let p_g = FixedGenerators::SpendingKeyGenerator;

    let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

    let tree_depth = params::BALANCE_TREE_DEPTH as u32;
    let mut tree = AccountTree::new(tree_depth);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << params::BALANCE_TREE_DEPTH);

    let sender_sk = PrivateKey::<Bn256>(rng.gen());
    let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
    let (sender_x, sender_y) = sender_pk.0.into_xy();

    // give some funds to sender and make zero balance for recipient

    // let sender_leaf_number = 1;

    let mut sender_leaf_number : u32 = rng.gen();
    sender_leaf_number = sender_leaf_number % capacity;
    
    let transfer_amount : u128 = 1234567890;

    let transfer_amount_as_field_element = Fr::from_str(&transfer_amount.to_string()).unwrap();

    let sender_leaf = Account {
            balance:    transfer_amount_as_field_element.clone(),
            nonce:      Fr::zero(),
            pub_x:      sender_x,
            pub_y:      sender_y,
    };

    tree.insert(sender_leaf_number, sender_leaf.clone());

    print!("Sender leaf hash is {}\n", tree.get_hash((tree_depth, sender_leaf_number)));

    //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));
    
    let initial_root = tree.root_hash();
    print!("Initial root = {}\n", initial_root);

    let path_from : Vec<Option<Fr>> = tree.merkle_path(sender_leaf_number).into_iter().map(|e| Some(e.0)).collect();

    let from = Fr::from_str(&sender_leaf_number.to_string());

    let request : ExitRequest<Bn256> = ExitRequest {
        from: from,
        amount: Some(transfer_amount_as_field_element)
    };

    let leaf_witness = LeafWitness {
        balance: Some(transfer_amount_as_field_element),
        nonce: Some(Fr::zero()),
        pub_x: Some(sender_x),
        pub_y: Some(sender_y),
    };

<<<<<<< HEAD
    let emptied_leaf = Account {
=======
    let empty_leaf_witness = LeafWitness {
        balance: Some(Fr::zero()),
        nonce: Some(Fr::zero()),
        pub_x: Some(Fr::zero()),
        pub_y: Some(Fr::zero()),
    };

    let witness = ExitWitness {
        leaf: leaf_witness,
        auth_path: path_from,
    };

    let emptied_leaf = BabyLeaf {
>>>>>>> more_ff
            balance:    Fr::zero(),
            nonce:      Fr::zero(),
            pub_x:      Fr::zero(),
            pub_y:      Fr::zero(),
    };

    tree.insert(sender_leaf_number, emptied_leaf);

    let new_root = tree.root_hash();

    print!("New root = {}\n", new_root);

    assert!(initial_root != new_root);

    {
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let mut public_data_initial_bits = vec![];

        // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

        let block_number_bits: Vec<bool> = BitIterator::new(Fr::one().into_repr()).collect();
        for _ in 0..256-block_number_bits.len() {
            public_data_initial_bits.push(false);
        }
        public_data_initial_bits.extend(block_number_bits.into_iter());

        assert_eq!(public_data_initial_bits.len(), 256);

        let mut h = Sha256::new();

        let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

        h.input(&bytes_to_hash);

        let mut hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        print!("Initial hash hex {}\n", hex::encode(hash_result.clone()));

        let mut packed_transaction_data = vec![];
        let transaction_data = request.public_data_into_bits();
        packed_transaction_data.extend(transaction_data.clone().into_iter());

        let leaf_bits = packed_transaction_data.clone();

        let packed_transaction_data_bytes = be_bit_vector_into_bytes(&packed_transaction_data);

        print!("Packed transaction data hex {}\n", hex::encode(packed_transaction_data_bytes.clone()));

        let mut next_round_hash_bytes = vec![];
        next_round_hash_bytes.extend(hash_result.iter());
        next_round_hash_bytes.extend(packed_transaction_data_bytes);

        h = Sha256::new();
        h.input(&next_round_hash_bytes);
        hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);

        print!("Final hash as hex {}\n", hex::encode(hash_result.clone()));

        hash_result[0] &= 0x1f; // temporary solution

        let mut repr = Fr::zero().into_repr();
        repr.read_be(&hash_result[..]).expect("pack hash as field element");

        let public_data_commitment = Fr::from_repr(repr).unwrap();

        print!("Final data commitment as field element = {}\n", public_data_commitment);

        let instance = Exit {
            params: params,
            number_of_exits: 1,
            old_root: Some(initial_root),
            new_root: Some(new_root),
            public_data_commitment: Some(public_data_commitment),
            empty_leaf_witness: empty_leaf_witness,
            block_number: Some(Fr::one()),
            requests: vec![(request, witness)],
        };

        instance.synthesize(&mut cs).unwrap();

        print!("{}\n", cs.find_unconstrained());

        print!("{}\n", cs.num_constraints());

        assert_eq!(cs.num_inputs(), 4);

        let err = cs.which_is_unsatisfied();
        if err.is_some() {
            panic!("ERROR satisfying in {}\n", err.unwrap());
        }
    }
}