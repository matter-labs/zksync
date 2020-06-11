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
        self.no_eth_signature().await?;
        self.incorrect_eth_signature().await?;

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
}
