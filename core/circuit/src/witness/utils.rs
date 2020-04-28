// External deps
use crypto::{digest::Digest, sha2::Sha256};
use crypto_exports::franklin_crypto::{
    alt_babyjubjub::AltJubjubBn256,
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::{BitIterator, Field, PrimeField, PrimeFieldRepr},
    },
    eddsa::{PrivateKey, PublicKey},
    jubjub::{FixedGenerators, JubjubEngine},
    rescue::bn256::Bn256RescueParams,
};
use crypto_exports::rand::{Rng, SeedableRng, XorShiftRng};
// Workspace deps
use models::{
    circuit::{
        account::{Balance, CircuitAccount, CircuitAccountTree},
        utils::{be_bit_vector_into_bytes, le_bit_vector_into_field_element},
    },
    merkle_tree::{hasher::Hasher, PedersenHasher, RescueHasher},
    node::{
        operations::{CloseOp, TransferOp, TransferToNewOp, WithdrawOp},
        tx::PackedPublicKey,
        AccountId, BlockNumber, Engine,
    },
    params as franklin_constants,
    params::total_tokens,
    primitives::big_decimal_to_u128,
};
use plasma::state::CollectedFee;
// Local deps
use crate::{
    account::AccountWitness,
    circuit::FranklinCircuit,
    operation::{Operation, SignatureData},
    utils::sign_rescue,
};

/// Wrapper around `CircuitAccountTree`
/// that simplifies witness generation
/// used for testing
pub struct WitnessBuilder<'a> {
    pub account_tree: &'a mut CircuitAccountTree,
    pub fee_account_id: AccountId,
    pub block_number: BlockNumber,
    pub initial_root_hash: Fr,
    pub operations: Vec<Operation<Engine>>,
    pub pubdata: Vec<bool>,
    pub root_before_fees: Option<Fr>,
    pub root_after_fees: Option<Fr>,
    pub fee_account_balances: Option<Vec<Option<Fr>>>,
    pub fee_account_witness: Option<AccountWitness<Engine>>,
    pub fee_account_audit_path: Option<Vec<Option<Fr>>>,
    pub pubdata_commitment: Option<Fr>,
}

impl<'a> WitnessBuilder<'a> {
    pub fn new(
        account_tree: &'a mut CircuitAccountTree,
        fee_account_id: AccountId,
        block_number: BlockNumber,
    ) -> WitnessBuilder {
        let initial_root_hash = account_tree.root_hash();
        WitnessBuilder {
            account_tree,
            fee_account_id,
            block_number,
            initial_root_hash,
            operations: Vec::new(),
            pubdata: Vec::new(),
            root_before_fees: None,
            root_after_fees: None,
            fee_account_balances: None,
            fee_account_witness: None,
            fee_account_audit_path: None,
            pubdata_commitment: None,
        }
    }

    /// Add witness generated for operation
    pub fn add_operation_with_pubdata(&mut self, ops: Vec<Operation<Engine>>, pubdata: Vec<bool>) {
        self.operations.extend(ops.into_iter());
        self.pubdata.extend(pubdata.into_iter());
    }

    /// Add noops if pubdata isn't of right size
    pub fn extend_pubdata_with_noops(&mut self, block_size_chunks: usize) {
        let chunks_used = self.operations.len();
        let chunks_remaining = block_size_chunks
            .checked_sub(chunks_used)
            .expect("failed to get number of noops");
        for _ in 0..chunks_remaining {
            self.operations.push(crate::witness::noop::noop_operation(
                &self.account_tree,
                self.fee_account_id,
            ));
            self.pubdata.extend(vec![false; 64]);
        }
    }

    /// After operations are added, collect fees.
    pub fn collect_fees(&mut self, fees: &[CollectedFee]) {
        self.root_before_fees = Some(self.account_tree.root_hash());

        let fee_circuit_account = self
            .account_tree
            .get(self.fee_account_id)
            .expect("fee account is not in the tree");
        let mut fee_circuit_account_balances = Vec::with_capacity(total_tokens());
        for i in 0u32..(total_tokens() as u32) {
            let balance_value = fee_circuit_account
                .subtree
                .get(i)
                .map(|bal| bal.value)
                .unwrap_or_else(Fr::zero);
            fee_circuit_account_balances.push(Some(balance_value));
        }
        self.fee_account_balances = Some(fee_circuit_account_balances);

        let (mut root_after_fee, mut fee_account_witness) =
            crate::witness::utils::apply_fee(&mut self.account_tree, self.fee_account_id, 0, 0);
        for CollectedFee { token, amount } in fees {
            let (root, acc_witness) = crate::witness::utils::apply_fee(
                &mut self.account_tree,
                self.fee_account_id,
                u32::from(*token),
                big_decimal_to_u128(amount),
            );
            root_after_fee = root;
            fee_account_witness = acc_witness;
        }

        self.root_after_fees = Some(root_after_fee);
        self.fee_account_witness = Some(fee_account_witness);
    }

    /// After fees collected creates public data commitment
    pub fn calculate_pubdata_commitment(&mut self) {
        let (fee_account_audit_path, _) =
            crate::witness::utils::get_audits(&self.account_tree, self.fee_account_id, 0);
        self.fee_account_audit_path = Some(fee_account_audit_path);

        let public_data_commitment = crate::witness::utils::public_data_commitment::<Engine>(
            &self.pubdata,
            Some(self.initial_root_hash),
            Some(
                self.root_after_fees
                    .expect("root after fee should be present at this step"),
            ),
            Some(Fr::from_str(&self.fee_account_id.to_string()).expect("failed to parse")),
            Some(Fr::from_str(&self.block_number.to_string()).unwrap()),
        );
        self.pubdata_commitment = Some(public_data_commitment);
    }

    /// Finaly, creates circuit instance for given operations.
    pub fn into_circuit_instance(self) -> FranklinCircuit<'static, Engine> {
        FranklinCircuit {
            rescue_params: &models::params::RESCUE_PARAMS,
            jubjub_params: &models::params::JUBJUB_PARAMS,
            old_root: Some(self.initial_root_hash),
            operations: self.operations,
            pub_data_commitment: Some(
                self.pubdata_commitment
                    .expect("pubdata commitment not present"),
            ),
            block_number: Some(Fr::from_str(&self.block_number.to_string()).unwrap()),
            validator_account: self
                .fee_account_witness
                .expect("fee account witness not present"),
            validator_address: Some(Fr::from_str(&self.fee_account_id.to_string()).unwrap()),
            validator_balances: self
                .fee_account_balances
                .expect("fee account balances not present"),
            validator_audit_path: self
                .fee_account_audit_path
                .expect("fee account audit path not present"),
        }
    }
}

pub fn generate_dummy_sig_data(
    bits: &[bool],
    rescue_hasher: &RescueHasher<Bn256>,
    rescue_params: &Bn256RescueParams,
    jubjub_params: &AltJubjubBn256,
) -> (SignatureData, Fr, Fr, Fr, Fr, Fr) {
    let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let private_key = PrivateKey::<Bn256>(rng.gen());
    let sender_pk = PublicKey::from_private(&private_key, p_g, &jubjub_params);
    let (sender_x, sender_y) = sender_pk.0.into_xy();
    let mut sig_bits_to_hash = bits.to_vec();
    assert!(sig_bits_to_hash.len() < franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS);

    sig_bits_to_hash.resize(franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS, false);
    let (first_sig_part_bits, remaining) = sig_bits_to_hash.split_at(Fr::CAPACITY as usize);
    let remaining = remaining.to_vec();
    let (second_sig_part_bits, third_sig_part_bits) = remaining.split_at(Fr::CAPACITY as usize);
    let first_sig_part: Fr = le_bit_vector_into_field_element(&first_sig_part_bits);
    let second_sig_part: Fr = le_bit_vector_into_field_element(&second_sig_part_bits);
    let third_sig_part: Fr = le_bit_vector_into_field_element(&third_sig_part_bits);
    let sig_msg = rescue_hasher.hash_bits(sig_bits_to_hash.clone());
    let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
    sig_bits.reverse();
    sig_bits.resize(256, false);

    let signature_data = sign_rescue(&sig_bits, &private_key, p_g, rescue_params, jubjub_params);
    (
        signature_data,
        first_sig_part,
        second_sig_part,
        third_sig_part,
        sender_x,
        sender_y,
    )
}

pub fn generate_sig_witness(
    bits: &[bool],
    _phasher: &PedersenHasher<Bn256>,
    _params: &AltJubjubBn256,
) -> (Fr, Fr, Fr) {
    let mut sig_bits_to_hash = bits.to_vec();
    assert!(sig_bits_to_hash.len() < franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS);

    sig_bits_to_hash.resize(franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS, false);
    let (first_sig_part_bits, remaining) = sig_bits_to_hash.split_at(Fr::CAPACITY as usize);
    let remaining = remaining.to_vec();
    let (second_sig_part_bits, third_sig_part_bits) = remaining.split_at(Fr::CAPACITY as usize);
    let first_sig_part: Fr = le_bit_vector_into_field_element(&first_sig_part_bits);
    let second_sig_part: Fr = le_bit_vector_into_field_element(&second_sig_part_bits);
    let third_sig_part: Fr = le_bit_vector_into_field_element(&third_sig_part_bits);
    (first_sig_part, second_sig_part, third_sig_part)
}

pub fn generate_sig_data(
    bits: &[bool],
    phasher: &PedersenHasher<Bn256>,
    private_key: &PrivateKey<Bn256>,
    rescue_params: &Bn256RescueParams,
    jubjub_params: &AltJubjubBn256,
) -> (SignatureData, Fr, Fr, Fr) {
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let mut sig_bits_to_hash = bits.to_vec();
    assert!(sig_bits_to_hash.len() <= franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS);

    sig_bits_to_hash.resize(franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS, false);
    debug!(
        "inside generation after resize: {}",
        hex::encode(be_bit_vector_into_bytes(&sig_bits_to_hash))
    );

    let (first_sig_part_bits, remaining) = sig_bits_to_hash.split_at(Fr::CAPACITY as usize);
    let remaining = remaining.to_vec();
    let (second_sig_part_bits, third_sig_part_bits) = remaining.split_at(Fr::CAPACITY as usize);
    let first_sig_part: Fr = le_bit_vector_into_field_element(&first_sig_part_bits);
    let second_sig_part: Fr = le_bit_vector_into_field_element(&second_sig_part_bits);
    let third_sig_part: Fr = le_bit_vector_into_field_element(&third_sig_part_bits);
    let sig_msg = phasher.hash_bits(sig_bits_to_hash.clone());

    let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
    sig_bits.reverse();
    sig_bits.resize(256, false);

    debug!(
        "inside generation: {}",
        hex::encode(be_bit_vector_into_bytes(&sig_bits))
    );
    let signature_data = sign_rescue(&sig_bits, &private_key, p_g, rescue_params, jubjub_params);

    (
        signature_data,
        first_sig_part,
        second_sig_part,
        third_sig_part,
    )
}

pub fn public_data_commitment<E: JubjubEngine>(
    pubdata_bits: &[bool],
    initial_root: Option<E::Fr>,
    new_root: Option<E::Fr>,
    validator_address: Option<E::Fr>,
    block_number: Option<E::Fr>,
) -> E::Fr {
    let mut public_data_initial_bits = vec![];

    // these two are BE encodings because an iterator is BE. This is also an Ethereum standard behavior

    let block_number_bits: Vec<bool> =
        BitIterator::new(block_number.unwrap().into_repr()).collect();
    for _ in 0..256 - block_number_bits.len() {
        public_data_initial_bits.push(false);
    }
    public_data_initial_bits.extend(block_number_bits.into_iter());

    let validator_id_bits: Vec<bool> =
        BitIterator::new(validator_address.unwrap().into_repr()).collect();
    for _ in 0..256 - validator_id_bits.len() {
        public_data_initial_bits.push(false);
    }
    public_data_initial_bits.extend(validator_id_bits.into_iter());

    assert_eq!(public_data_initial_bits.len(), 512);

    let mut h = Sha256::new();

    let bytes_to_hash = be_bit_vector_into_bytes(&public_data_initial_bits);

    h.input(&bytes_to_hash);

    let mut hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    debug!("Initial hash hex {}", hex::encode(hash_result));

    let mut packed_old_root_bits = vec![];
    let old_root_bits: Vec<bool> = BitIterator::new(initial_root.unwrap().into_repr()).collect();
    for _ in 0..256 - old_root_bits.len() {
        packed_old_root_bits.push(false);
    }
    packed_old_root_bits.extend(old_root_bits);

    let packed_old_root_bytes = be_bit_vector_into_bytes(&packed_old_root_bits);

    let mut packed_with_old_root = vec![];
    packed_with_old_root.extend(hash_result.iter());
    packed_with_old_root.extend(packed_old_root_bytes);

    h = Sha256::new();
    h.input(&packed_with_old_root);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    let mut packed_new_root_bits = vec![];
    let new_root_bits: Vec<bool> = BitIterator::new(new_root.unwrap().into_repr()).collect();
    for _ in 0..256 - new_root_bits.len() {
        packed_new_root_bits.push(false);
    }
    packed_new_root_bits.extend(new_root_bits);

    let packed_new_root_bytes = be_bit_vector_into_bytes(&packed_new_root_bits);

    let mut packed_with_new_root = vec![];
    packed_with_new_root.extend(hash_result.iter());
    packed_with_new_root.extend(packed_new_root_bytes);

    h = Sha256::new();
    h.input(&packed_with_new_root);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    debug!("hash with new root as hex {}", hex::encode(hash_result));

    let mut final_bytes = vec![];
    let pubdata_bytes = be_bit_vector_into_bytes(&pubdata_bits.to_vec());
    final_bytes.extend(hash_result.iter());
    final_bytes.extend(pubdata_bytes);

    h = Sha256::new();
    h.input(&final_bytes);
    hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);

    debug!("final hash as hex {}", hex::encode(hash_result));

    hash_result[0] &= 0x1f; // temporary solution, this nullifies top bits to be encoded into field element correctly

    let mut repr = E::Fr::zero().into_repr();
    repr.read_be(&hash_result[..])
        .expect("pack hash as field element");

    E::Fr::from_repr(repr).unwrap()
}

pub fn get_audits(
    tree: &CircuitAccountTree,
    account_address: u32,
    token: u32,
) -> (Vec<Option<Fr>>, Vec<Option<Fr>>) {
    let default_account = CircuitAccount::default();
    let audit_account: Vec<Option<Fr>> = tree
        .merkle_path(account_address)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();

    let audit_balance: Vec<Option<Fr>> = tree
        .get(account_address)
        .unwrap_or(&default_account)
        .subtree
        .merkle_path(token)
        .into_iter()
        .map(|e| Some(e.0))
        .collect();
    (audit_account, audit_balance)
}

pub fn apply_leaf_operation<
    Fa: Fn(&mut CircuitAccount<Bn256>) -> (),
    Fb: Fn(&mut Balance<Bn256>) -> (),
>(
    tree: &mut CircuitAccountTree,
    account_address: u32,
    token: u32,
    fa: Fa,
    fb: Fb,
) -> (AccountWitness<Bn256>, AccountWitness<Bn256>, Fr, Fr) {
    let default_account = CircuitAccount::default();

    //applying deposit
    let mut account = tree.remove(account_address).unwrap_or(default_account);
    let account_witness_before = AccountWitness::from_circuit_account(&account);
    let mut balance = account
        .subtree
        .remove(token)
        .unwrap_or(Balance { value: Fr::zero() });
    let balance_before = balance.value;
    fb(&mut balance);
    let balance_after = balance.value;
    account.subtree.insert(token, balance);

    fa(&mut account);

    let account_witness_after = AccountWitness::from_circuit_account(&account);
    tree.insert(account_address, account);
    (
        account_witness_before,
        account_witness_after,
        balance_before,
        balance_after,
    )
}

pub fn apply_fee(
    tree: &mut CircuitAccountTree,
    validator_address: u32,
    token: u32,
    fee: u128,
) -> (Fr, AccountWitness<Bn256>) {
    let fee_fe = Fr::from_str(&fee.to_string()).unwrap();
    let mut validator_leaf = tree
        .remove(validator_address)
        .expect("validator_leaf is empty");
    let validator_account_witness = AccountWitness::from_circuit_account(&validator_leaf);

    let mut balance = validator_leaf.subtree.remove(token).unwrap_or_default();
    balance.value.add_assign(&fee_fe);
    validator_leaf.subtree.insert(token, balance);

    tree.insert(validator_address, validator_leaf);

    let root_after_fee = tree.root_hash();
    (root_after_fee, validator_account_witness)
}

pub fn fr_from_bytes(bytes: Vec<u8>) -> Fr {
    let mut fr_repr = <Fr as PrimeField>::Repr::default();
    fr_repr.read_be(&*bytes).unwrap();
    Fr::from_repr(fr_repr).unwrap()
}

/// Gathered signature data for calculating the operations in several
/// witness structured (e.g. `TransferWitness` or `WithdrawWitness`).
#[derive(Debug, Clone)]
pub struct SigDataInput {
    pub first_sig_msg: Fr,
    pub second_sig_msg: Fr,
    pub third_sig_msg: Fr,
    pub signature: SignatureData,
    pub signer_pub_key_packed: Vec<Option<bool>>,
}

impl SigDataInput {
    /// Creates a new `SigDataInput` from the raw tx contents, signature and public key
    /// of the author.
    pub fn new(
        sig_bytes: &[u8],
        tx_bytes: &[u8],
        pub_key: &PackedPublicKey,
    ) -> Result<SigDataInput, String> {
        let (r_bytes, s_bytes) = sig_bytes.split_at(32);
        let r_bits: Vec<_> = models::primitives::bytes_into_be_bits(&r_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();
        let s_bits: Vec<_> = models::primitives::bytes_into_be_bits(&s_bytes)
            .iter()
            .map(|x| Some(*x))
            .collect();
        let signature = SignatureData {
            r_packed: r_bits,
            s: s_bits,
        };
        let sig_bits: Vec<bool> = models::primitives::bytes_into_be_bits(&tx_bytes);

        let (first_sig_msg, second_sig_msg, third_sig_msg) = self::generate_sig_witness(
            &sig_bits,
            &models::params::PEDERSEN_HASHER,
            &models::params::JUBJUB_PARAMS,
        );

        let signer_packed_key_bytes = match pub_key.serialize_packed() {
            Ok(v) => v,
            Err(e) => {
                return Err(format!("failed to prepare signature data: {}", e));
            }
        };
        let signer_pub_key_packed: Vec<_> =
            models::primitives::bytes_into_be_bits(&signer_packed_key_bytes)
                .iter()
                .map(|x| Some(*x))
                .collect();
        Ok(SigDataInput {
            first_sig_msg,
            second_sig_msg,
            third_sig_msg,
            signature,
            signer_pub_key_packed,
        })
    }

    pub fn from_close_op(close_op: &CloseOp) -> Result<Self, String> {
        let sign_packed = close_op
            .tx
            .signature
            .signature
            .serialize_packed()
            .expect("signature serialize");
        let input = SigDataInput::new(
            &sign_packed,
            &close_op.tx.get_bytes(),
            &close_op.tx.signature.pub_key,
        );

        input
    }

    pub fn from_transfer_op(transfer_op: &TransferOp) -> Result<Self, String> {
        let sign_packed = transfer_op
            .tx
            .signature
            .signature
            .serialize_packed()
            .expect("signature serialize");
        let input = SigDataInput::new(
            &sign_packed,
            &transfer_op.tx.get_bytes(),
            &transfer_op.tx.signature.pub_key,
        );

        input
    }

    pub fn from_transfer_to_new_op(transfer_op: &TransferToNewOp) -> Result<Self, String> {
        let sign_packed = transfer_op
            .tx
            .signature
            .signature
            .serialize_packed()
            .expect("signature serialize");
        let input = SigDataInput::new(
            &sign_packed,
            &transfer_op.tx.get_bytes(),
            &transfer_op.tx.signature.pub_key,
        );

        input
    }

    pub fn from_withdraw_op(withdraw_op: &WithdrawOp) -> Result<Self, String> {
        let sign_packed = withdraw_op
            .tx
            .signature
            .signature
            .serialize_packed()
            .expect("signature serialize");
        let input = SigDataInput::new(
            &sign_packed,
            &withdraw_op.tx.get_bytes(),
            &withdraw_op.tx.signature.pub_key,
        );

        input
    }

    /// Provides a vector of copies of this `SigDataInput` object, all with one field
    /// set to incorrect value.
    /// Used for circuit tests.
    #[cfg(test)]
    pub fn corrupted_variations(&self) -> Vec<Self> {
        let incorrect_fr = crate::witness::tests::test_utils::incorrect_fr();
        vec![
            SigDataInput {
                first_sig_msg: incorrect_fr,
                ..self.clone()
            },
            SigDataInput {
                second_sig_msg: incorrect_fr,
                ..self.clone()
            },
            SigDataInput {
                third_sig_msg: incorrect_fr,
                ..self.clone()
            },
            SigDataInput {
                signature: SignatureData::init_empty(),
                ..self.clone()
            },
            SigDataInput {
                signer_pub_key_packed: vec![Some(false); self.signer_pub_key_packed.len()],
                ..self.clone()
            },
        ]
    }
}
