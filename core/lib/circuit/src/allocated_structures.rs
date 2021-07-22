// External deps
use zksync_crypto::franklin_crypto::{
    bellman::{
        pairing::{ff::PrimeField, Engine},
        ConstraintSystem, SynthesisError,
    },
    circuit::{
        boolean::Boolean, float_point::parse_with_exponent_le, num::AllocatedNum, Assignment,
    },
    rescue::RescueEngine,
};
// Workspace deps
use zksync_crypto::params as franklin_constants;
// Local deps
use crate::{
    account::{self, AccountContent},
    element::CircuitElement,
    operation::{Operation, OperationBranch},
    utils,
};

pub struct AllocatedOperationBranch<E: RescueEngine> {
    pub account: AccountContent<E>,
    pub account_audit_path: Vec<AllocatedNum<E>>, //we do not need their bit representations
    pub account_id: CircuitElement<E>,
    pub balance: CircuitElement<E>,
    pub balance_audit_path: Vec<AllocatedNum<E>>,
    pub token: CircuitElement<E>,
}

impl<E: RescueEngine> AllocatedOperationBranch<E> {
    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        operation_branch: &OperationBranch<E>,
    ) -> Result<AllocatedOperationBranch<E>, SynthesisError> {
        let account_address = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "account_address"),
            || operation_branch.address.grab(),
            franklin_constants::account_tree_depth(),
        )?;
        let account_address = account_address.pad(franklin_constants::ACCOUNT_ID_BIT_WIDTH);

        let account_audit_path = utils::allocate_numbers_vec(
            cs.namespace(|| "account_audit_path"),
            &operation_branch.witness.account_path,
        )?;
        assert_eq!(
            account_audit_path.len(),
            franklin_constants::account_tree_depth()
        );

        let account = account::AccountContent::from_witness(
            cs.namespace(|| "allocate account_content"),
            &operation_branch.witness.account_witness,
        )?;

        let balance = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "balance"),
            || operation_branch.witness.balance_value.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        let token = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "token"),
            || operation_branch.token.grab(),
            franklin_constants::balance_tree_depth(),
        )?;
        let token = token.pad(franklin_constants::TOKEN_BIT_WIDTH);
        let balance_audit_path = utils::allocate_numbers_vec(
            cs.namespace(|| "balance_audit_path"),
            &operation_branch.witness.balance_subtree_path,
        )?;
        assert_eq!(
            balance_audit_path.len(),
            franklin_constants::balance_tree_depth()
        );

        Ok(AllocatedOperationBranch {
            account,
            account_audit_path,
            account_id: account_address,
            balance,
            token,
            balance_audit_path,
        })
    }
}

pub struct AllocatedChunkData<E: Engine> {
    pub is_chunk_last: Boolean,
    pub is_chunk_first: Boolean,
    pub chunk_number: AllocatedNum<E>,
    pub tx_type: CircuitElement<E>,
}

#[derive(Clone)]
pub struct AllocatedOperationData<E: Engine> {
    pub amount_packed: CircuitElement<E>,
    pub fee_packed: CircuitElement<E>,
    pub amount_unpacked: CircuitElement<E>,

    pub second_amount_packed: CircuitElement<E>,
    pub second_amount_unpacked: CircuitElement<E>,
    pub special_amounts_packed: Vec<CircuitElement<E>>,
    pub special_amounts_unpacked: Vec<CircuitElement<E>>,
    pub special_prices: Vec<CircuitElement<E>>,
    pub special_nonces: Vec<CircuitElement<E>>,
    pub special_accounts: Vec<CircuitElement<E>>,
    pub special_eth_addresses: Vec<CircuitElement<E>>,

    pub special_tokens: Vec<CircuitElement<E>>,
    pub special_content_hash: Vec<CircuitElement<E>>,
    pub special_serial_id: CircuitElement<E>,
    pub full_amount: CircuitElement<E>,
    pub fee: CircuitElement<E>,
    pub first_sig_msg: CircuitElement<E>,
    pub second_sig_msg: CircuitElement<E>,
    pub third_sig_msg: CircuitElement<E>,
    pub new_pubkey_hash: CircuitElement<E>,
    pub eth_address: CircuitElement<E>,
    pub pub_nonce: CircuitElement<E>,
    pub a: CircuitElement<E>,
    pub b: CircuitElement<E>,
    pub valid_from: CircuitElement<E>,
    pub valid_until: CircuitElement<E>,
    pub second_valid_from: CircuitElement<E>,
    pub second_valid_until: CircuitElement<E>,
}

impl<E: RescueEngine> AllocatedOperationData<E> {
    pub fn empty_from_zero(zero_element: AllocatedNum<E>) -> Result<Self, SynthesisError> {
        let eth_address = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
        );

        let full_amount = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let amount_packed = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH
                + franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        );

        let special_token = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );

        let special_account_id = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );

        let special_content_hash =
            vec![
                CircuitElement::unsafe_empty_of_some_length(zero_element.clone(), 1,);
                franklin_constants::CONTENT_HASH_WIDTH
            ];

        let special_serial_id = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::SERIAL_ID_WIDTH,
        );

        let fee_packed = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::FEE_EXPONENT_BIT_WIDTH + franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        );

        let amount_unpacked = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let price_part = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::PRICE_BIT_WIDTH,
        );

        let fee = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let first_sig_msg = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            E::Fr::CAPACITY as usize,
        );

        let second_sig_msg = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            E::Fr::CAPACITY as usize,
        );

        let third_sig_msg = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS - (2 * E::Fr::CAPACITY as usize), //TODO: think of more consistent constant flow (ZKS-54).
        );

        let new_pubkey_hash = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );

        let pub_nonce = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::NONCE_BIT_WIDTH,
        );

        let a = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let b = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        let valid_from = CircuitElement::unsafe_empty_of_some_length(
            zero_element.clone(),
            franklin_constants::TIMESTAMP_BIT_WIDTH,
        );

        let valid_until = CircuitElement::unsafe_empty_of_some_length(
            zero_element,
            franklin_constants::TIMESTAMP_BIT_WIDTH,
        );

        Ok(AllocatedOperationData {
            amount_packed: amount_packed.clone(),
            fee_packed,
            amount_unpacked: amount_unpacked.clone(),
            second_amount_packed: amount_packed.clone(),
            second_amount_unpacked: amount_unpacked.clone(),
            special_amounts_packed: vec![amount_packed; 2],
            special_amounts_unpacked: vec![amount_unpacked; 2],
            special_prices: vec![price_part; 4],
            special_nonces: vec![pub_nonce.clone(); 3],
            special_accounts: vec![special_account_id; 5],
            special_eth_addresses: vec![eth_address.clone(); 2],
            special_tokens: vec![special_token; 3],
            special_content_hash,
            special_serial_id,
            full_amount,
            fee,
            first_sig_msg,
            second_sig_msg,
            third_sig_msg,
            eth_address,
            new_pubkey_hash,
            pub_nonce,
            a,
            b,
            valid_from: valid_from.clone(),
            valid_until: valid_until.clone(),
            second_valid_from: valid_from,
            second_valid_until: valid_until,
        })
    }

    fn get_amounts<CS: ConstraintSystem<E>>(
        mut cs: CS,
        amount: Option<E::Fr>,
    ) -> Result<(CircuitElement<E>, CircuitElement<E>), SynthesisError> {
        let amount_packed = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "amount_packed"),
            || amount.grab(),
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH
                + franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
        )?;
        let amount_parsed = parse_with_exponent_le(
            cs.namespace(|| "parse amount"),
            &amount_packed.get_bits_le(),
            franklin_constants::AMOUNT_EXPONENT_BIT_WIDTH,
            franklin_constants::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )?;
        let amount_unpacked = CircuitElement::from_number_with_known_length(
            cs.namespace(|| "amount_unpacked"),
            amount_parsed,
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        Ok((amount_packed, amount_unpacked))
    }

    pub fn from_witness<CS: ConstraintSystem<E>>(
        mut cs: CS,
        op: &Operation<E>,
    ) -> Result<AllocatedOperationData<E>, SynthesisError> {
        macro_rules! parse_circuit_elements {
            ($element:ident, $len:expr) => {
                let $element = op
                    .args
                    .$element
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| {
                        CircuitElement::from_fe_with_known_length(
                            cs.namespace(|| {
                                format!("{} item with index {}", stringify!($element), idx)
                            }),
                            || item.grab(),
                            $len,
                        )
                    })
                    .collect::<Result<Vec<_>, SynthesisError>>()?;
            };
        }

        let eth_address = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "eth_address"),
            || op.args.eth_address.grab(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
        )?;

        parse_circuit_elements!(special_content_hash, 1);
        parse_circuit_elements!(special_tokens, franklin_constants::TOKEN_BIT_WIDTH);
        parse_circuit_elements!(special_accounts, franklin_constants::ACCOUNT_ID_BIT_WIDTH);
        parse_circuit_elements!(special_nonces, franklin_constants::NONCE_BIT_WIDTH);
        parse_circuit_elements!(special_prices, franklin_constants::PRICE_BIT_WIDTH);
        parse_circuit_elements!(
            special_eth_addresses,
            franklin_constants::ETH_ADDRESS_BIT_WIDTH
        );

        let special_serial_id = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "special_serial_id"),
            || op.args.special_serial_id.grab(),
            franklin_constants::SERIAL_ID_WIDTH,
        )?;

        let full_amount = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "full_amount"),
            || op.args.full_amount.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        let (amount_packed, amount_unpacked) =
            Self::get_amounts(cs.namespace(|| "get amount"), op.args.amount_packed)?;

        let (second_amount_packed, second_amount_unpacked) = Self::get_amounts(
            cs.namespace(|| "get second amount"),
            op.args.second_amount_packed,
        )?;

        let (special_amounts_packed, special_amounts_unpacked) = op
            .args
            .special_amounts
            .iter()
            .enumerate()
            .map(|(idx, &special_amount)| {
                Self::get_amounts(
                    cs.namespace(|| format!("special_amount with index {}", idx)),
                    special_amount,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .unzip();

        let fee_packed = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "fee_packed"),
            || op.args.fee.grab(),
            franklin_constants::FEE_EXPONENT_BIT_WIDTH + franklin_constants::FEE_MANTISSA_BIT_WIDTH,
        )?;

        let fee_parsed = parse_with_exponent_le(
            cs.namespace(|| "parse fee"),
            &fee_packed.get_bits_le(),
            franklin_constants::FEE_EXPONENT_BIT_WIDTH,
            franklin_constants::FEE_MANTISSA_BIT_WIDTH,
            10,
        )?;
        let fee = CircuitElement::from_number_with_known_length(
            cs.namespace(|| "fee"),
            fee_parsed,
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;

        let first_sig_msg = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "first_part_signature_message"),
            || op.first_sig_msg.grab(),
            E::Fr::CAPACITY as usize,
        )?;

        let second_sig_msg = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "second_part_signature_message"),
            || op.second_sig_msg.grab(),
            E::Fr::CAPACITY as usize, //TODO: think of more consistent constant flow (ZKS-54).
        )?;

        let third_sig_msg = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "third_part_signature_message"),
            || op.third_sig_msg.grab(),
            franklin_constants::MAX_CIRCUIT_MSG_HASH_BITS - (2 * E::Fr::CAPACITY as usize), //TODO: think of more consistent constant flow (ZKS-54).
        )?;

        let new_pubkey_hash = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "new_pubkey_hash"),
            || op.args.new_pub_key_hash.grab(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        )?;

        let pub_nonce = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "pub_nonce"),
            || op.args.pub_nonce.grab(),
            franklin_constants::NONCE_BIT_WIDTH,
        )?;
        let a = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "a"),
            || op.args.a.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        let b = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "b"),
            || op.args.b.grab(),
            franklin_constants::BALANCE_BIT_WIDTH,
        )?;
        let valid_from = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "valid_from"),
            || op.args.valid_from.grab(),
            franklin_constants::TIMESTAMP_BIT_WIDTH,
        )?;
        let valid_until = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "valid_until"),
            || op.args.valid_until.grab(),
            franklin_constants::TIMESTAMP_BIT_WIDTH,
        )?;
        let second_valid_from = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "second_valid_from"),
            || op.args.second_valid_from.grab(),
            franklin_constants::TIMESTAMP_BIT_WIDTH,
        )?;
        let second_valid_until = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "second_valid_until"),
            || op.args.second_valid_until.grab(),
            franklin_constants::TIMESTAMP_BIT_WIDTH,
        )?;

        Ok(AllocatedOperationData {
            amount_packed,
            fee_packed,
            amount_unpacked,
            second_amount_packed,
            second_amount_unpacked,
            special_amounts_packed,
            special_amounts_unpacked,
            special_prices,
            special_nonces,
            special_accounts,
            special_eth_addresses,
            special_tokens,
            special_content_hash,
            special_serial_id,
            full_amount,
            fee,
            first_sig_msg,
            second_sig_msg,
            third_sig_msg,
            new_pubkey_hash,
            eth_address,
            pub_nonce,
            a,
            b,
            valid_from,
            valid_until,
            second_valid_from,
            second_valid_until,
        })
    }
}
