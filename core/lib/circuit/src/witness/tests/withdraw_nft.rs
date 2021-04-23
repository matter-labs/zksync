// External deps
use num::BigUint;
use zksync_crypto::params::{MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID};
// Workspace deps
use zksync_state::{handler::TxHandler, state::ZkSyncState};
use zksync_types::{
    operations::{MintNFTOp, WithdrawNFTOp},
    AccountId, BlockNumber, MintNFT, TokenId, WithdrawNFT, H256,
};
// Local deps
use crate::witness::{
    tests::test_utils::{check_circuit, WitnessTestAccount, ZkSyncStateGenerator, FEE_ACCOUNT_ID},
    utils::{SigDataInput, WitnessBuilder},
    MintNFTWitness, WithdrawNFTWitness, Witness,
};

/// Basic check for execution of `WithdrawNFT` operation in circuit.
/// Here we create two accounts and perform mintNFT and withdrawNFT operations.
#[test]
#[ignore]
fn test_mint_and_withdraw_nft() {
    let accounts = vec![
        WitnessTestAccount::new(AccountId(1), 10u64), // nft creator account
        WitnessTestAccount::new(AccountId(2), 10u64), // account to withdraw nft
        WitnessTestAccount::new_with_token(
            NFT_STORAGE_ACCOUNT_ID,
            NFT_TOKEN_ID,
            MIN_NFT_TOKEN_ID as u64,
        ),
    ];

    let nft_content_hash = H256::random();

    // Mint NFT.
    let mint_nft_op = MintNFTOp {
        tx: accounts[0]
            .zksync_account
            .sign_mint_nft(
                TokenId(0),
                "",
                nft_content_hash,
                BigUint::from(10u64),
                &accounts[1].account.address,
                None,
                true,
            )
            .0,
        creator_account_id: accounts[0].id,
        recipient_account_id: accounts[1].id,
    };
    let mint_nft_input =
        SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

    // Withdraw NFT.
    let withdraw_nft_op = WithdrawNFTOp {
        tx: accounts[1]
            .zksync_account
            .sign_withdraw_nft(
                TokenId(MIN_NFT_TOKEN_ID),
                TokenId(0),
                "",
                BigUint::from(10u64),
                &accounts[1].account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        account_id: accounts[1].id,
        creator_id: accounts[0].id,
        creator_address: accounts[0].zksync_account.address,
        content_hash: nft_content_hash,
        serial_id: 0,
    };
    let withdraw_nft_input =
        SigDataInput::from_withdraw_nft_op(&withdraw_nft_op).expect("SigDataInput creation failed");

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);
    let mut witness_accum =
        WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, BlockNumber(1), 0);

    // Fees to be collected.
    let mut fees = vec![];

    // Apply MintNFT op.
    let fee = <ZkSyncState as TxHandler<MintNFT>>::apply_op(&mut plasma_state, &mint_nft_op)
        .expect("Operation failed")
        .0
        .unwrap();
    fees.push(fee);

    let witness = MintNFTWitness::apply_tx(&mut witness_accum.account_tree, &mint_nft_op);
    let circuit_operations = witness.calculate_operations(mint_nft_input);
    let pub_data_from_witness = witness.get_pubdata();
    let offset_commitment = witness.get_offset_commitment_data();

    witness_accum.add_operation_with_pubdata(
        circuit_operations,
        pub_data_from_witness,
        offset_commitment,
    );

    // Apply WithdrawNFT op.
    let fee =
        <ZkSyncState as TxHandler<WithdrawNFT>>::apply_op(&mut plasma_state, &withdraw_nft_op)
            .expect("Operation failed")
            .0
            .unwrap();
    fees.push(fee);

    let witness = WithdrawNFTWitness::apply_tx(&mut witness_accum.account_tree, &withdraw_nft_op);
    let circuit_operations = witness.calculate_operations(withdraw_nft_input);
    let pub_data_from_witness = witness.get_pubdata();
    let offset_commitment = witness.get_offset_commitment_data();

    witness_accum.add_operation_with_pubdata(
        circuit_operations,
        pub_data_from_witness,
        offset_commitment,
    );

    // Collect fees.
    plasma_state.collect_fee(&fees, FEE_ACCOUNT_ID);
    witness_accum.collect_fees(&fees);
    witness_accum.calculate_pubdata_commitment();

    // Check that root hashes match
    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    // Verify that there are no unsatisfied constraints
    check_circuit(witness_accum.into_circuit_instance());
}
