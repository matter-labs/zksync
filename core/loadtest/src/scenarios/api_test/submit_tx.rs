//! Tests for `submit_tx` RPC method.

// External deps
use jsonrpc_core::types::{Failure, Output};
use num::BigUint;
// Workspace deps
use models::node::{
    tx::{PackedEthSignature, Transfer, TxSignature},
    Address, FranklinTx, TokenId,
};
use server::api_server::rpc_server::RpcErrorCodes;
use testkit::zksync_account::ZksyncAccount;
// Local deps
use super::TestExecutor;

pub struct SubmitTxTester<'a>(&'a TestExecutor);

impl<'a> SubmitTxTester<'a> {
    pub fn new(executor: &'a TestExecutor) -> Self {
        Self(executor)
    }

    pub async fn run(self) -> Result<(), failure::Error> {
        TestExecutor::execute_test("No ethereum signature", || self.no_eth_signature()).await;
        TestExecutor::execute_test("Incorrect ethereum signature", || {
            self.incorrect_eth_signature()
        })
        .await;
        TestExecutor::execute_test("Too low fee", || self.low_fee()).await;
        TestExecutor::execute_test("Incorrect account ID", || self.incorrect_account_id()).await;
        TestExecutor::execute_test("Unpackable token amount", || self.unpackable_token_amount())
            .await;
        TestExecutor::execute_test("Unpackable fee amount", || self.unpackable_fee_amount()).await;
        // TestExecutor::execute_test("Too big token amount", || self.too_big_token_amount()).await;
        // TestExecutor::execute_test("Too big fee amount", || self.too_big_fee_amount()).await;

        Ok(())
    }

    fn check_rpc_code(&self, output: Failure, expected_code: RpcErrorCodes) {
        if output.error.code != expected_code.into() {
            panic!(
                "Expected RPC response: {:?}; Actual RPC response: {:?}",
                expected_code, output
            );
        }
    }

    pub async fn no_eth_signature(&self) {
        let main_account = &self.0.main_account;

        let transfer_fee = self.0.transfer_fee(&main_account.zk_acc).await;

        let (transfer, _) = self.0.sign_transfer(
            &main_account.zk_acc,
            &main_account.zk_acc,
            1u32,
            transfer_fee,
        );

        // Discard ETH signature.
        let no_eth_sign = None;

        let expected_error = RpcErrorCodes::MissingEthSignature;
        self.check_incorrect_transfer_response(transfer, no_eth_sign, expected_error)
            .await;
    }

    pub async fn incorrect_eth_signature(&self) {
        let main_account = &self.0.main_account;

        let transfer_fee = self.0.transfer_fee(&main_account.zk_acc).await;

        let (transfer, _) = self.0.sign_transfer(
            &main_account.zk_acc,
            &main_account.zk_acc,
            1u32,
            transfer_fee,
        );

        // Replace ETH signature with an incorrect one.
        let fake_signature =
            PackedEthSignature::deserialize_packed(&[0; 65]).expect("Can't deserialize signature");
        let eth_sign = Some(fake_signature);

        let expected_error = RpcErrorCodes::IncorrectEthSignature;
        self.check_incorrect_transfer_response(transfer, eth_sign, expected_error)
            .await;
    }

    pub async fn low_fee(&self) {
        let main_account = &self.0.main_account;

        // Set fee to 0.
        let transfer_fee = 0u32;

        let (transfer, eth_sign) = self.0.sign_transfer(
            &main_account.zk_acc,
            &main_account.zk_acc,
            1u32,
            transfer_fee,
        );

        let expected_error = RpcErrorCodes::FeeTooLow;
        self.check_incorrect_transfer_response(transfer, eth_sign, expected_error)
            .await;
    }

    pub async fn incorrect_account_id(&self) {
        // Make random sender with incorrect account ID.
        let incorrect_account_id = u32::max_value();
        let random_account = ZksyncAccount::rand();
        random_account.set_account_id(Some(incorrect_account_id));

        let transfer_fee = self.0.transfer_fee(&random_account).await;

        let (transfer, eth_sign) = Self::sign_transfer(
            &random_account,
            random_account.address,
            10_u32.into(),
            transfer_fee,
        );

        let expected_error = RpcErrorCodes::IncorrectTx;
        self.check_incorrect_transfer_response(transfer, eth_sign, expected_error)
            .await;
    }

    pub async fn unpackable_token_amount(&self) {
        let main_account = &self.0.main_account;
        let transfer_fee = self.0.transfer_fee(&main_account.zk_acc).await;

        let unpackable_token_amount = 1_000_000_000_000_000_001u64.into();

        let (transfer, eth_sign) = Self::sign_transfer(
            &main_account.zk_acc,
            main_account.zk_acc.address,
            unpackable_token_amount,
            transfer_fee,
        );

        let expected_error = RpcErrorCodes::IncorrectTx;
        self.check_incorrect_transfer_response(transfer, eth_sign, expected_error)
            .await;
    }

    pub async fn unpackable_fee_amount(&self) {
        let main_account = &self.0.main_account;

        let unpackable_fee_amount = 1_000_000_000_000_000_001u64.into();

        let (transfer, eth_sign) = Self::sign_transfer(
            &main_account.zk_acc,
            main_account.zk_acc.address,
            10u32.into(),
            unpackable_fee_amount,
        );

        let expected_error = RpcErrorCodes::IncorrectTx;
        self.check_incorrect_transfer_response(transfer, eth_sign, expected_error)
            .await;
    }

    // pub async fn incorrect_signature(&self) -> Result<(), failure::Error> {

    // }

    // pub async fn too_big_token_amount(&self) -> Result<(), failure::Error> {
    //     let main_account = &self.0.main_account;
    //     let transfer_fee = self.0.transfer_fee(&main_account.zk_acc).await;

    //     let too_big_transfer_amount = BigUint::from(u128::max_value()) + BigUint::from(1u32);

    //     let (transfer, eth_sign) = Self::sign_transfer(
    //         &main_account.zk_acc,
    //         main_account.zk_acc.address,
    //         too_big_transfer_amount,
    //         transfer_fee,
    //     );

    //     let reply = self.0.rpc_client.send_tx_raw(transfer, eth_sign).await?;
    //     match reply {
    //         Output::Success(v) => {
    //             panic!(
    //                 "Got successful response for tx with too big token amount: {:?}",
    //                 v
    //             );
    //         }
    //         Output::Failure(v) => {
    //             self.check_rpc_code(v, RpcErrorCodes::IncorrectTx.into());
    //         }
    //     };

    //     Ok(())
    // }

    // pub async fn too_big_fee_amount(&self) -> Result<(), failure::Error> {
    //     let main_account = &self.0.main_account;

    //     let too_big_fee_amount = BigUint::from(u128::max_value()) + BigUint::from(1u32);

    //     let (transfer, eth_sign) = Self::sign_transfer(
    //         &main_account.zk_acc,
    //         main_account.zk_acc.address,
    //         0u32.into(),
    //         too_big_fee_amount,
    //     );

    //     let reply = self.0.rpc_client.send_tx_raw(transfer, eth_sign).await?;
    //     match reply {
    //         Output::Success(v) => {
    //             panic!(
    //                 "Got successful response for tx with too big fee amount: {:?}",
    //                 v
    //             );
    //         }
    //         Output::Failure(v) => {
    //             self.check_rpc_code(v, RpcErrorCodes::IncorrectTx.into());
    //         }
    //     };

    //     Ok(())
    // }

    /// Sends the transaction and expects it to fail with a provided RPC error code.
    async fn check_incorrect_transfer_response(
        &self,
        transfer: FranklinTx,
        eth_sign: Option<PackedEthSignature>,
        expected_error: RpcErrorCodes,
    ) {
        let reply = self
            .0
            .rpc_client
            .send_tx_raw(transfer, eth_sign)
            .await
            .expect("Can't send the transaction");
        match reply {
            Output::Success(v) => {
                panic!("Got successful response for incorrect tx: {:?}", v);
            }
            Output::Failure(v) => {
                self.check_rpc_code(v, expected_error.into());
            }
        };
    }

    /// Creates signed transfer without any checks for correctness.
    fn sign_transfer(
        from: &ZksyncAccount,
        to: Address,
        amount: BigUint,
        fee: BigUint,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        let token: TokenId = 0; // ETH token
        let account_id = from.get_account_id().expect("Account ID must be set");
        let mut tx = Transfer::new(
            account_id,
            from.address,
            to,
            token,
            amount,
            fee,
            from.nonce(),
            None,
        );
        tx.signature = TxSignature::sign_musig(&from.private_key, &tx.get_bytes());

        let eth_signature = PackedEthSignature::sign(
            &from.eth_private_key,
            tx.get_ethereum_sign_message("ETH").as_bytes(),
        )
        .expect("Signing the transfer unexpectedly failed");

        (FranklinTx::Transfer(Box::new(tx)), Some(eth_signature))
    }
}
