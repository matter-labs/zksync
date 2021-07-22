// External deps
use num::ToPrimitive;

use zksync_crypto::franklin_crypto::{
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::{Field, PrimeField},
    },
    rescue::RescueEngine,
};
// Workspace deps
use zksync_crypto::{
    circuit::{
        account::CircuitAccountTree,
        utils::{append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element},
    },
    params::{
        account_tree_depth, ACCOUNT_ID_BIT_WIDTH, CHUNK_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH,
        FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, NFT_STORAGE_ACCOUNT_ID, SERIAL_ID_WIDTH,
        TOKEN_BIT_WIDTH, TX_TYPE_BIT_WIDTH,
    },
    primitives::FloatConversions,
};
use zksync_types::operations::WithdrawNFTOp;
use zksync_types::H256;
// Local deps
use crate::witness::utils::fr_from;
use crate::{
    operation::{Operation, OperationArguments, OperationBranch, OperationBranchWitness},
    utils::resize_grow_only,
    witness::{
        utils::{apply_leaf_operation, get_audits, SigDataInput},
        Witness,
    },
};

#[derive(Debug)]
pub struct WithdrawNFTData {
    pub fee: u128,
    pub fee_token: u32,
    pub initiator_account_id: u32,
    pub creator_account_id: u32,
    pub nft_serial_id: u32,
    pub content_hash: H256,
    pub token: u32,
    pub to_address: Fr,
    pub valid_from: u64,
    pub valid_until: u64,
}

pub struct WithdrawNFTWitness<E: RescueEngine> {
    pub before_second_chunk_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,

    pub tx_type: Option<E::Fr>,
    pub args: OperationArguments<E>,

    pub initiator_before_first_chunk: OperationBranch<E>,
    pub initiator_before_second_chunk: OperationBranch<E>,
    pub special_account_third_chunk: OperationBranch<E>,
    pub creator_account_fourth_chunk: OperationBranch<E>,
}

impl Witness for WithdrawNFTWitness<Bn256> {
    type OperationType = WithdrawNFTOp;
    type CalculateOpsInput = SigDataInput;

    fn apply_tx(tree: &mut CircuitAccountTree, withdraw_nft: &WithdrawNFTOp) -> Self {
        let time_range = withdraw_nft.tx.time_range;
        let withdraw_nft_data = WithdrawNFTData {
            fee: withdraw_nft.tx.fee.to_u128().unwrap(),
            fee_token: *withdraw_nft.tx.fee_token as u32,
            initiator_account_id: *withdraw_nft.tx.account_id,
            creator_account_id: *withdraw_nft.creator_id,
            nft_serial_id: withdraw_nft.serial_id,
            content_hash: withdraw_nft.content_hash,
            token: *withdraw_nft.tx.token as u32,
            to_address: eth_address_to_fr(&withdraw_nft.tx.to),
            valid_from: time_range.valid_from,
            valid_until: time_range.valid_until,
        };
        Self::apply_data(tree, &withdraw_nft_data)
    }

    fn get_pubdata(&self) -> Vec<bool> {
        // construct pubdata
        let mut pubdata_bits = vec![];
        append_be_fixed_width(&mut pubdata_bits, &self.tx_type.unwrap(), TX_TYPE_BIT_WIDTH);

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.special_accounts[1].unwrap(),
            ACCOUNT_ID_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.special_accounts[0].unwrap(),
            ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.special_eth_addresses[0].unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.special_serial_id.unwrap(),
            SERIAL_ID_WIDTH,
        );
        for bit in &self.args.special_content_hash {
            append_be_fixed_width(&mut pubdata_bits, &bit.unwrap(), 1);
        }

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.special_tokens[1].unwrap(),
            TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.special_tokens[0].unwrap(),
            TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            FEE_MANTISSA_BIT_WIDTH + FEE_EXPONENT_BIT_WIDTH,
        );

        resize_grow_only(
            &mut pubdata_bits,
            WithdrawNFTOp::CHUNKS * CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }

    fn get_offset_commitment_data(&self) -> Vec<bool> {
        let mut commitment = vec![false; WithdrawNFTOp::CHUNKS * 8];
        commitment[7] = true;
        commitment
    }

    fn calculate_operations(&self, input: SigDataInput) -> Vec<Operation<Bn256>> {
        let pubdata_chunks: Vec<_> = self
            .get_pubdata()
            .chunks(CHUNK_BIT_WIDTH)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect();

        let first_chunk = Operation {
            new_root: self.before_second_chunk_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("0").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[0]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.initiator_before_first_chunk.clone(),
            rhs: self.initiator_before_first_chunk.clone(),
        };
        let second_chunk = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("1").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[1]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.initiator_before_second_chunk.clone(),
            rhs: self.initiator_before_second_chunk.clone(),
        };
        let third_chunk = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("2").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[2]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.special_account_third_chunk.clone(),
            rhs: self.special_account_third_chunk.clone(),
        };
        let fourth_chunk = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("3").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[3]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.creator_account_fourth_chunk.clone(),
            rhs: self.creator_account_fourth_chunk.clone(),
        };
        let rest_chunks = (4..WithdrawNFTOp::CHUNKS).map(|chunk| Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str(&chunk.to_string()).unwrap()),
            pubdata_chunk: Some(pubdata_chunks[chunk]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.creator_account_fourth_chunk.clone(),
            rhs: self.creator_account_fourth_chunk.clone(),
        });
        vec![first_chunk, second_chunk, third_chunk, fourth_chunk]
            .into_iter()
            .chain(rest_chunks)
            .collect()
    }
}

impl WithdrawNFTWitness<Bn256> {
    fn apply_data(tree: &mut CircuitAccountTree, withdraw_nft: &WithdrawNFTData) -> Self {
        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << account_tree_depth());

        let initiator_account_id_fe =
            Fr::from_str(&withdraw_nft.initiator_account_id.to_string()).unwrap();
        let creator_account_id_fe =
            Fr::from_str(&withdraw_nft.creator_account_id.to_string()).unwrap();
        let fee_token_fe = Fr::from_str(&withdraw_nft.fee_token.to_string()).unwrap();
        let token_fe = Fr::from_str(&withdraw_nft.token.to_string()).unwrap();
        let serial_id_fe = Fr::from_str(&withdraw_nft.nft_serial_id.to_string()).unwrap();

        let fee_as_field_element = Fr::from_str(&withdraw_nft.fee.to_string()).unwrap();
        let fee_bits = FloatConversions::to_float(
            withdraw_nft.fee,
            FEE_EXPONENT_BIT_WIDTH,
            FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();
        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        let valid_from = withdraw_nft.valid_from;
        let valid_until = withdraw_nft.valid_until;

        let before_first_chunk_root = tree.root_hash();
        vlog::debug!("Initial root = {}", before_first_chunk_root);

        // applying first chunk: take fee from initiator, increment nonce
        let (
            audit_initiator_account_before_first_chunk,
            audit_initiator_balance_before_first_chunk,
        ) = get_audits(
            tree,
            withdraw_nft.initiator_account_id,
            withdraw_nft.fee_token,
        );

        let (
            initiator_account_witness_before_first_chunk,
            _initiator_account_witness_after_first_chunk,
            fee_balance_before_first_chunk,
            _fee_balance_after_first_chunk,
        ) = apply_leaf_operation(
            tree,
            withdraw_nft.initiator_account_id,
            withdraw_nft.fee_token,
            |acc| {
                acc.nonce.add_assign(&Fr::from_str("1").unwrap());
            },
            |bal| {
                bal.value.sub_assign(&fee_as_field_element);
            },
        );

        let (
            _audit_initiator_account_after_first_chunk,
            _audit_initiator_balance_after_first_chunk,
        ) = get_audits(
            tree,
            withdraw_nft.initiator_account_id,
            withdraw_nft.fee_token,
        );

        let before_second_chunk_root = tree.root_hash();
        vlog::debug!("Before second chunk root = {}", before_second_chunk_root);

        // applying second chunk: nullify the balance of the initiator
        let (
            audit_initiator_account_before_second_chunk,
            audit_initiator_balance_before_second_chunk,
        ) = get_audits(tree, withdraw_nft.initiator_account_id, withdraw_nft.token);

        let (
            initiator_account_witness_before_second_chunk,
            _initiator_account_witness_after_second_chunk,
            token_balance_before_second_chunk,
            _token_balance_after_second_chunk,
        ) = apply_leaf_operation(
            tree,
            withdraw_nft.initiator_account_id,
            withdraw_nft.token,
            |_| {},
            |bal| {
                bal.value.sub_assign(&Fr::from_str("1").unwrap());
            },
        );

        let (
            _audit_initiator_account_after_second_chunk,
            _audit_initiator_balance_after_second_chunk,
        ) = get_audits(tree, withdraw_nft.initiator_account_id, withdraw_nft.token);

        // third chunk
        let (audit_special_account_third_chunk, audit_special_balance_third_chunk) =
            get_audits(tree, NFT_STORAGE_ACCOUNT_ID.0, withdraw_nft.token);

        let (
            special_account_witness_third_chunk,
            _special_account_witness_third_chunk,
            special_account_balance_third_chunk,
            _special_account_balance_third_chunk,
        ) = apply_leaf_operation(
            tree,
            NFT_STORAGE_ACCOUNT_ID.0,
            withdraw_nft.token,
            |_| {},
            |_| {},
        );

        // fourth chunk
        let (audit_creator_account_fourth_chunk, audit_creator_balance_fourth_chunk) =
            get_audits(tree, withdraw_nft.creator_account_id, 0);

        let (
            creator_account_witness_fourth_chunk,
            _creator_account_witness_fourth_chunk,
            creator_account_balance_fourth_chunk,
            _creator_account_balance_fourth_chunk,
        ) = apply_leaf_operation(tree, withdraw_nft.creator_account_id, 0, |_| {}, |_| {});

        let after_root = tree.root_hash();
        vlog::debug!("After root = {}", after_root);

        let a = fee_balance_before_first_chunk;
        let b = fee_as_field_element;

        let content_hash_as_vec: Vec<Option<Fr>> = withdraw_nft
            .content_hash
            .as_bytes()
            .iter()
            .map(|input_byte| {
                let mut byte_as_bits = vec![];
                let mut byte = *input_byte;
                for _ in 0..8 {
                    byte_as_bits.push(byte & 1);
                    byte /= 2;
                }
                byte_as_bits.reverse();
                byte_as_bits
            })
            .flatten()
            .map(|bit| Some(fr_from(&bit)))
            .collect();

        WithdrawNFTWitness {
            before_second_chunk_root: Some(before_second_chunk_root),
            after_root: Some(after_root),

            tx_type: Some(Fr::from_str(&WithdrawNFTOp::OP_CODE.to_string()).unwrap()),
            args: OperationArguments {
                eth_address: Some(withdraw_nft.to_address),
                fee: Some(fee_encoded),
                a: Some(a),
                b: Some(b),
                valid_from: Some(fr_from(&valid_from)),
                valid_until: Some(fr_from(&valid_until)),
                special_eth_addresses: vec![
                    Some(
                        creator_account_witness_fourth_chunk
                            .address
                            .expect("creator account should not be empty"),
                    ),
                    Some(Fr::zero()),
                ],
                special_tokens: vec![Some(fee_token_fe), Some(token_fe), Some(Fr::zero())],
                special_accounts: vec![
                    Some(creator_account_id_fe),
                    Some(initiator_account_id_fe),
                    Some(Fr::zero()),
                    Some(Fr::zero()),
                    Some(Fr::zero()),
                ],
                special_content_hash: content_hash_as_vec,
                special_serial_id: Some(serial_id_fe),
                ..Default::default()
            },

            initiator_before_first_chunk: OperationBranch {
                address: Some(initiator_account_id_fe),
                token: Some(fee_token_fe),
                witness: OperationBranchWitness {
                    account_witness: initiator_account_witness_before_first_chunk,
                    account_path: audit_initiator_account_before_first_chunk,
                    balance_value: Some(fee_balance_before_first_chunk),
                    balance_subtree_path: audit_initiator_balance_before_first_chunk,
                },
            },
            initiator_before_second_chunk: OperationBranch {
                address: Some(initiator_account_id_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: initiator_account_witness_before_second_chunk,
                    account_path: audit_initiator_account_before_second_chunk,
                    balance_value: Some(token_balance_before_second_chunk),
                    balance_subtree_path: audit_initiator_balance_before_second_chunk,
                },
            },
            special_account_third_chunk: OperationBranch {
                address: Some(fr_from(&NFT_STORAGE_ACCOUNT_ID.0)),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: special_account_witness_third_chunk,
                    account_path: audit_special_account_third_chunk,
                    balance_value: Some(special_account_balance_third_chunk),
                    balance_subtree_path: audit_special_balance_third_chunk,
                },
            },
            creator_account_fourth_chunk: OperationBranch {
                address: Some(creator_account_id_fe),
                token: Some(Fr::zero()),
                witness: OperationBranchWitness {
                    account_witness: creator_account_witness_fourth_chunk,
                    account_path: audit_creator_account_fourth_chunk,
                    balance_value: Some(creator_account_balance_fourth_chunk),
                    balance_subtree_path: audit_creator_balance_fourth_chunk,
                },
            },
        }
    }
}
