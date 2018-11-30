extern crate rand;
extern crate pairing;
extern crate sapling_crypto;
extern crate ff;
extern crate hex;
extern crate crypto;
extern crate plasma;
extern crate time;
extern crate bellman;

use time::PreciseTime;

use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};
use pairing::bn256::*;
use rand::{SeedableRng, Rng, XorShiftRng, Rand};
use sapling_crypto::circuit::test::*;
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256, fs, PrimeOrder};
use plasma::balance_tree::{BabyBalanceTree, BabyLeaf, Leaf};
use crypto::sha2::Sha256;
use crypto::digest::Digest;
use std::collections::HashMap;

use bellman::{
    SynthesisError,
    ConstraintSystem,
    Circuit,
};

use bellman::groth16::{
    create_random_proof, 
    generate_random_parameters, 
    prepare_verifying_key, 
    verify_proof,
};

use sapling_crypto::jubjub::{
    JubjubEngine,
    FixedGenerators,
    Unknown,
    edwards,
    JubjubParams
};

use sapling_crypto::eddsa::{
    Signature,
    PrivateKey,
    PublicKey
};

use plasma::circuit::plasma_constants;
use plasma::circuit::baby_plasma::{Transaction, TransactionWitness, Update, le_bit_vector_into_field_element, be_bit_vector_into_bytes};
use sapling_crypto::circuit::float_point::{convert_to_float};
use sapling_crypto::circuit::baby_eddsa::EddsaSignature;

fn main() {
    let TXES_TO_TEST = 5;

    let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &AltJubjubBn256::new();
    let mut rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let tree_depth = *plasma_constants::BALANCE_TREE_DEPTH as u32;

    let capacity: u32 = 1 << tree_depth;

    let mut existing_accounts: Vec<(u32, PrivateKey::<Bn256>, PublicKey::<Bn256>)> = vec![];

    let mut tree = BabyBalanceTree::new(tree_depth);

    let number_of_accounts = 100;

    let existing_account_hm = HashMap::<u32, bool>::new();

    let default_balance_string = "1000000";
    let transfer_amount : u128 = 1000;
    let fee_amount : u128 = 0;

    for _ in 0..number_of_accounts {
        let mut leaf_number : u32 = rng.gen();
        leaf_number = leaf_number % capacity;
        if existing_account_hm.get(&leaf_number).is_some() {
            continue;
        }

        let sk = PrivateKey::<Bn256>(rng.gen());
        let pk = PublicKey::from_private(&sk, p_g, params);
        let (x, y) = pk.0.into_xy();

        existing_accounts.push((leaf_number, sk, pk));

        let leaf = BabyLeaf {
            balance:    Fr::from_str(default_balance_string).unwrap(),
            nonce:      Fr::zero(),
            pub_x:      x,
            pub_y:      y,
        };

        tree.insert(leaf_number, leaf.clone());
    }

    let num_accounts = existing_accounts.len();

    println!("Inserted {} accounts", num_accounts);

    let initial_root = tree.root_hash();

    println!("Starting with a root of {}", initial_root);

    let mut witnesses: Vec<Option<(Transaction<Bn256>, TransactionWitness<Bn256>)>> = vec![];
    let mut public_data_vector: Vec<Vec<bool>> = vec![];

    let transfer_amount_as_field_element = Fr::from_str(&transfer_amount.to_string()).unwrap();

    let transfer_amount_bits = convert_to_float(
        transfer_amount,
        *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH,
        *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        10
    ).unwrap();

    let transfer_amount_encoded: Fr = le_bit_vector_into_field_element(&transfer_amount_bits);

    let fee_as_field_element = Fr::from_str(&fee_amount.to_string()).unwrap();

    let fee_bits = convert_to_float(
        fee_amount,
        *plasma_constants::FEE_EXPONENT_BIT_WIDTH,
        *plasma_constants::FEE_MANTISSA_BIT_WIDTH,
        10
    ).unwrap();

    let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

    let mut total_fees = Fr::zero();

    for _ in 0..TXES_TO_TEST {
        let mut sender_account_number: usize = rng.gen();
        sender_account_number = sender_account_number % num_accounts;
        let sender_account_info: &(u32, PrivateKey::<Bn256>, PublicKey::<Bn256>) = existing_accounts.get(sender_account_number).clone().unwrap();

        let mut recipient_account_number: usize = rng.gen();
        recipient_account_number = recipient_account_number % num_accounts;
        if recipient_account_number == sender_account_number {
            recipient_account_number = recipient_account_number+1 % num_accounts;
        }
        let recipient_account_info: &(u32, PrivateKey::<Bn256>, PublicKey::<Bn256>) = existing_accounts.get(recipient_account_number).clone().unwrap();

        let sender_leaf_number = sender_account_info.0;
        let recipient_leaf_number = recipient_account_info.0;

        let path_from : Vec<Option<(Fr, bool)>> = tree.merkle_path(sender_leaf_number).into_iter().map(|e| Some(e)).collect();
        let path_to: Vec<Option<(Fr, bool)>>  = tree.merkle_path(recipient_leaf_number).into_iter().map(|e| Some(e)).collect();

        let from = Fr::from_str(& sender_leaf_number.to_string());
        let to = Fr::from_str(& recipient_leaf_number.to_string());

        let mut transaction : Transaction<Bn256> = Transaction {
            from: from,
            to: to,
            amount: Some(transfer_amount_encoded.clone()),
            fee: Some(fee_encoded.clone()),
            nonce: Some(Fr::zero()),
            good_until_block: Some(Fr::one()),
            signature: None
        };

        let sender_sk = &sender_account_info.1;

        transaction.sign(
            &sender_sk,
            p_g,
            params,
            rng
        );

        assert!(transaction.signature.is_some());

        let mut items = tree.items.clone();

        let sender_leaf = items.get(&sender_leaf_number).unwrap().clone();
        let recipient_leaf = items.get(&recipient_leaf_number).unwrap().clone();

        let mut updated_sender_leaf = sender_leaf.clone();
        let mut updated_recipient_leaf = recipient_leaf.clone();

        updated_sender_leaf.balance.sub_assign(&transfer_amount_as_field_element);
        updated_sender_leaf.balance.sub_assign(&fee_as_field_element);

        updated_sender_leaf.nonce.add_assign(&Fr::one());

        updated_recipient_leaf.balance.add_assign(&transfer_amount_as_field_element);

        total_fees.add_assign(&fee_as_field_element);

        tree.insert(sender_leaf_number, updated_sender_leaf.clone());
        tree.insert(recipient_leaf_number, updated_recipient_leaf.clone());

        let public_data = transaction.public_data_into_bits();
        public_data_vector.push(public_data);

        let transaction_witness = TransactionWitness {
            auth_path_from: path_from,
            balance_from: Some(sender_leaf.balance),
            nonce_from: Some(sender_leaf.nonce),
            pub_x_from: Some(sender_leaf.pub_x),
            pub_y_from: Some(sender_leaf.pub_y),
            auth_path_to: path_to,
            balance_to: Some(recipient_leaf.balance),
            nonce_to: Some(recipient_leaf.nonce),
            pub_x_to: Some(recipient_leaf.pub_x),
            pub_y_to: Some(recipient_leaf.pub_y)
        };

        let witness = (transaction.clone(), transaction_witness);

        witnesses.push(Some(witness));
    }

    let block_number = Fr::one();

    let final_root = tree.root_hash();

    let mut public_data_initial_bits = vec![];

    // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

    let block_number_bits: Vec<bool> = BitIterator::new(block_number.into_repr()).collect();
    for _ in 0..256-block_number_bits.len() {
        public_data_initial_bits.push(false);
    }
    public_data_initial_bits.extend(block_number_bits.into_iter());

    let total_fee_bits: Vec<bool> = BitIterator::new(total_fees.into_repr()).collect();
    for _ in 0..256-total_fee_bits.len() {
        public_data_initial_bits.push(false);
    }
    public_data_initial_bits.extend(total_fee_bits.into_iter());

    assert_eq!(public_data_initial_bits.len(), 512);

    let mut h = Sha256::new();

    let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

    h.input(&bytes_to_hash);

    let mut hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    let public_data_size = *plasma_constants::BALANCE_TREE_DEPTH 
                                    + *plasma_constants::BALANCE_TREE_DEPTH
                                    + *plasma_constants::AMOUNT_EXPONENT_BIT_WIDTH
                                    + *plasma_constants::AMOUNT_MANTISSA_BIT_WIDTH
                                    + *plasma_constants::FEE_EXPONENT_BIT_WIDTH
                                    + *plasma_constants::FEE_MANTISSA_BIT_WIDTH;

    let pack_by = 256 / public_data_size;

    let number_of_packs = TXES_TO_TEST / pack_by;
    let remaining_to_pack = TXES_TO_TEST % pack_by;
    let padding_in_pack = 256 - pack_by*public_data_size;
    let padding_in_remainder = 256 - remaining_to_pack*public_data_size;

    let mut public_data_iterator = public_data_vector.into_iter();

    for i in 0..number_of_packs 
    {

        let mut packed_transaction_data = vec![];
        
        for _ in 0..pack_by 
        {
            let transaction_data = public_data_iterator.next().unwrap();
            packed_transaction_data.extend(transaction_data.clone().into_iter());
        }
        for _ in 0..padding_in_pack
        {
            packed_transaction_data.push(false);
        }

        let packed_transaction_data_bytes = be_bit_vector_into_bytes(&packed_transaction_data);

        let mut next_round_hash_bytes = vec![];
        next_round_hash_bytes.extend(hash_result.iter());
        next_round_hash_bytes.extend(packed_transaction_data_bytes);
        assert_eq!(next_round_hash_bytes.len(), 64);

        h = Sha256::new();

        h.input(&next_round_hash_bytes);

        let mut hash_result = [0u8; 32];
        h.result(&mut hash_result[..]);
    }

    // now pack the remainder

    let mut packed_transaction_data = vec![];
        
    for _ in 0..remaining_to_pack 
    {
        let transaction_data = public_data_iterator.next().unwrap();
        packed_transaction_data.extend(transaction_data.clone().into_iter());
    }
    for _ in 0..padding_in_remainder
    {
        packed_transaction_data.push(false);
    }

    let packed_transaction_data_bytes = be_bit_vector_into_bytes(&packed_transaction_data);

    let mut next_round_hash_bytes = vec![];
    next_round_hash_bytes.extend(hash_result.iter());
    next_round_hash_bytes.extend(packed_transaction_data_bytes);
    assert_eq!(next_round_hash_bytes.len(), 64);

    h = Sha256::new();

    h.input(&next_round_hash_bytes);

    let mut hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    // clip to fit into field element

    hash_result[0] &= 0x1f; // temporary solution

    let mut repr = Fr::zero().into_repr();
    repr.read_be(&hash_result[..]).expect("pack hash as field element");

    let public_data_commitment = Fr::from_repr(repr).unwrap();


    let instance = Update {
        params: params,
        number_of_transactions: TXES_TO_TEST,
        old_root: Some(initial_root),
        new_root: Some(final_root),
        public_data_commitment: Some(public_data_commitment),
        block_number: Some(Fr::one()),
        total_fee: Some(total_fees),
        transactions: witnesses.clone(),
    };

    // let mut cs = TestConstraintSystem::<Bn256>::new();
    // instance.synthesize(&mut cs).unwrap();

    // print!("{}\n", cs.num_constraints());

    // assert_eq!(cs.num_inputs(), 4);

    // let err = cs.which_is_unsatisfied();
    // if err.is_some() {
    //     print!("ERROR satisfying in {}\n", err.unwrap());
    // } else {
    //     assert!(cs.is_satisfied());
    // }

    // assert!(cs.is_satisfied());

    // return;

    println!("generating setup...");
    let start = PreciseTime::now();
    let cirtuit_params_some = generate_random_parameters(instance, rng);
    if cirtuit_params_some.is_err() {
        println!("generating parameters ended with error");
    }
    let cirtuit_params = cirtuit_params_some.unwrap();
    println!("setup generated in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);

    let pvk = prepare_verifying_key(&cirtuit_params.vk);

    let proving_instance = Update {
        params: params,
        number_of_transactions: TXES_TO_TEST,
        old_root: Some(initial_root),
        new_root: Some(final_root),
        public_data_commitment: Some(public_data_commitment),
        block_number: Some(Fr::one()),
        total_fee: Some(total_fees),
        transactions: witnesses,
    };

    println!("creating proof...");
    let start = PreciseTime::now();
    let proof = create_random_proof(proving_instance, &cirtuit_params, rng).unwrap();
    println!("proof created in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);

    let success = verify_proof(&pvk, &proof, &[]).unwrap();
    assert!(success);

}
