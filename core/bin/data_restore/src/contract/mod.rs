use crate::eth_tx_helpers::get_input_data_from_ethereum_transaction;
use crate::rollup_ops::RollupOpsBlock;
use ethabi::{ParamType, Token};
use std::convert::TryFrom;
use web3::api::Eth;
use web3::contract::Options;
use web3::types::{Address, BlockId, BlockNumber, U256};
use web3::Transport;

use zksync_contracts::{
    zksync_contract, zksync_contract_v0, zksync_contract_v1, zksync_contract_v2, zksync_contract_v3,
};
use zksync_types::ZkSyncOp;

pub use crate::contract::utils::get_genesis_account;
pub use crate::contract::version::ZkSyncContractVersion;

pub mod default;
pub mod utils;
pub mod v4;
pub mod version;

#[derive(Debug)]
pub struct ZkSyncDeployedContract<T: Transport> {
    pub web3_contract: web3::contract::Contract<T>,
    pub abi: ethabi::Contract,
    pub version: ZkSyncContractVersion,
    pub from: BlockNumber,
    pub to: BlockNumber,
}

impl<T: Transport> ZkSyncDeployedContract<T> {
    /// Returns total number of verified blocks on Rollup contract
    ///
    /// # Arguments
    ///
    /// * `web3` - Web3 provider url
    /// * `zksync_contract` - Rollup contract
    ///
    pub async fn get_total_verified_blocks(&self) -> u32 {
        use ZkSyncContractVersion::*;
        let func = match self.version {
            V0 | V1 | V2 | V3 => "totalBlocksVerified",
            V4 => "totalBlocksExecuted",
        };
        self.web3_contract
            .query::<U256, Option<Address>, Option<BlockId>, ()>(
                func,
                (),
                None,
                Options::default(),
                None,
            )
            .await
            .unwrap()
            .as_u32()
    }

    pub fn version0(
        eth: Eth<T>,
        address: Address,
        from: BlockNumber,
        to: BlockNumber,
    ) -> ZkSyncDeployedContract<T> {
        let abi = zksync_contract_v0();
        ZkSyncDeployedContract {
            web3_contract: web3::contract::Contract::new(eth, address, abi.clone()),
            abi,
            version: ZkSyncContractVersion::V0,
            from,
            to,
        }
    }
    pub fn version1(
        eth: Eth<T>,
        address: Address,
        from: BlockNumber,
        to: BlockNumber,
    ) -> ZkSyncDeployedContract<T> {
        let abi = zksync_contract_v1();
        ZkSyncDeployedContract {
            web3_contract: web3::contract::Contract::new(eth, address, abi.clone()),
            abi,
            version: ZkSyncContractVersion::V1,
            from,
            to,
        }
    }
    pub fn version2(
        eth: Eth<T>,
        address: Address,
        from: BlockNumber,
        to: BlockNumber,
    ) -> ZkSyncDeployedContract<T> {
        let abi = zksync_contract_v2();
        ZkSyncDeployedContract {
            web3_contract: web3::contract::Contract::new(eth, address, abi.clone()),
            abi,
            version: ZkSyncContractVersion::V2,
            from,
            to,
        }
    }
    pub fn version3(
        eth: Eth<T>,
        address: Address,
        from: BlockNumber,
        to: BlockNumber,
    ) -> ZkSyncDeployedContract<T> {
        let abi = zksync_contract_v3();
        ZkSyncDeployedContract {
            web3_contract: web3::contract::Contract::new(eth, address, abi.clone()),
            abi,
            version: ZkSyncContractVersion::V3,
            from,
            to,
        }
    }
    pub fn version4(
        eth: Eth<T>,
        address: Address,
        from: BlockNumber,
        to: BlockNumber,
    ) -> ZkSyncDeployedContract<T> {
        let abi = zksync_contract();
        ZkSyncDeployedContract {
            web3_contract: web3::contract::Contract::new(eth, address, abi.clone()),
            abi,
            version: ZkSyncContractVersion::V4,
            from,
            to,
        }
    }
}
