//! Tests for `submit_tx` RPC method.

// External deps
use jsonrpc_core::types::response::Output;
// Workspace deps
use models::node::tx::PackedEthSignature;
use server::api_server::rpc_server::RpcErrorCodes;
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

        Ok(())
    }

    pub async fn no_eth_signature(&self) -> Result<(), failure::Error> {
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

        let reply = self.0.rpc_client.send_tx_raw(transfer, no_eth_sign).await?;
        match reply {
            Output::Success(v) => {
                panic!("Got successful response for tx with no signature: {:?}", v);
            }
            Output::Failure(v) => {
                assert_eq!(v.error.code, RpcErrorCodes::MissingEthSignature.into());
            }
        };

        Ok(())
    }

    pub async fn incorrect_eth_signature(&self) -> Result<(), failure::Error> {
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

        let reply = self.0.rpc_client.send_tx_raw(transfer, eth_sign).await?;
        match reply {
            Output::Success(v) => {
                panic!(
                    "Got successful response for an incorrect signature: {:?}",
                    v
                );
            }
            Output::Failure(v) => {
                assert_eq!(v.error.code, RpcErrorCodes::IncorrectEthSignature.into());
            }
        };

        Ok(())
    }

    pub async fn low_fee(&self) -> Result<(), failure::Error> {
        let main_account = &self.0.main_account;

        // Set fee to 0.
        let transfer_fee = 0u32;

        let (transfer, eth_sign) = self.0.sign_transfer(
            &main_account.zk_acc,
            &main_account.zk_acc,
            1u32,
            transfer_fee,
        );

        let reply = self.0.rpc_client.send_tx_raw(transfer, eth_sign).await?;
        match reply {
            Output::Success(v) => {
                panic!("Got successful response for tx with too low fee: {:?}", v);
            }
            Output::Failure(v) => {
                assert_eq!(v.error.code, RpcErrorCodes::FeeTooLow.into());
            }
        };

        Ok(())
    }
}
