use crate::eth_tx_helpers::get_input_data_from_ethereum_transaction;
use crate::rollup_ops::RollupOpsBlock;
use ethabi::{ParamType, Token};
use std::convert::TryFrom;
use web3::api::Eth;
use web3::contract::Options;
use web3::types::{Address, BlockId, BlockNumber, Transaction, U256};
use web3::Transport;
use zksync_contracts::{
    zksync_contract, zksync_contract_v0, zksync_contract_v1, zksync_contract_v2, zksync_contract_v3,
};
use zksync_crypto::params::{INPUT_DATA_ADDRESS_BYTES_WIDTH, INPUT_DATA_ROOT_HASH_BYTES_WIDTH};
use zksync_types::account::Account;
use zksync_types::ZkSyncOp;

/// Returns Rollup genesis (fees) account from the input of the Rollup contract creation transaction
///
/// # Arguments
///
/// * `transaction` - Ethereum Rollup contract creation transaction description
///
pub fn get_genesis_account(genesis_transaction: &Transaction) -> Result<Account, anyhow::Error> {
    const ENCODED_INIT_PARAMETERS_WIDTH: usize =
        6 * INPUT_DATA_ADDRESS_BYTES_WIDTH + INPUT_DATA_ROOT_HASH_BYTES_WIDTH;

    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;

    // Input for contract constructor contains the bytecode of the contract and
    // encoded arguments after it.
    // We are not interested in the bytecode and we know the size of arguments,
    // so we can simply cut the parameters bytes from the end of input array,
    // and then decode them to access required data.
    let encoded_init_parameters =
        input_data[input_data.len() - ENCODED_INIT_PARAMETERS_WIDTH..].to_vec();

    // Constructor parameters:
    // ```
    // constructor(
    //    Governance _govTarget, Verifier _verifierTarget, ZkSync _zkSyncTarget,
    //    bytes32 _genesisRoot, address _firstValidator, address _governor,
    //    address _feeAccountAddress
    // )
    let init_parameters_types = vec![
        ethabi::ParamType::Address, // Governance contract address
        ethabi::ParamType::Address, // Verifier contract address
        ethabi::ParamType::Address, // zkSync contract address
        ethabi::ParamType::FixedBytes(INPUT_DATA_ROOT_HASH_BYTES_WIDTH), // Genesis root
        ethabi::ParamType::Address, // First validator (committer) address
        ethabi::ParamType::Address, // Governor address
        ethabi::ParamType::Address, // Fee account address
    ];
    let fee_account_address_argument_id = 6;

    let decoded_init_parameters = ethabi::decode(
        init_parameters_types.as_slice(),
        encoded_init_parameters.as_slice(),
    )
    .map_err(|_| {
        anyhow::Error::from(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded init parameters from contract creation transaction",
        )))
    })?;
    match &decoded_init_parameters[fee_account_address_argument_id] {
        ethabi::Token::Address(genesis_operator_address) => {
            Some(Account::default_with_address(&genesis_operator_address))
        }
        _ => None,
    }
    .ok_or(Err("Invalid token in parameters"))
    .map_err(|_: Result<Account, _>| {
        anyhow::Error::from(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded init parameter from contract creation transaction",
        )))
    })
}

/// Returns total number of verified blocks on Rollup contract
///
/// # Arguments
///
/// * `web3` - Web3 provider url
/// * `zksync_contract` - Rollup contract
///

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkSyncContractVersion {
    V0,
    V1,
    V2,
    V3,
    V4,
}

impl TryFrom<u32> for ZkSyncContractVersion {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        use ZkSyncContractVersion::*;
        let res = match value {
            0 => Ok(V0),
            1 => Ok(V1),
            2 => Ok(V2),
            3 => Ok(V3),
            4 => Ok(V4),
            _ => Err(anyhow::anyhow!("Unsupported contract version")),
        };
        res
    }
}
impl Into<i32> for ZkSyncContractVersion {
    fn into(self) -> i32 {
        match self {
            ZkSyncContractVersion::V0 => 0,
            ZkSyncContractVersion::V1 => 1,
            ZkSyncContractVersion::V2 => 2,
            ZkSyncContractVersion::V3 => 3,
            ZkSyncContractVersion::V4 => 4,
        }
    }
}

pub struct ZkSyncDeployedContract<T: Transport> {
    pub web3_contract: web3::contract::Contract<T>,
    pub abi: ethabi::Contract,
    pub version: ZkSyncContractVersion,
    pub from: BlockNumber,
    pub to: BlockNumber,
}

impl<T: Transport> ZkSyncDeployedContract<T> {
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
    pub fn version4(eth: Eth<T>, address: Address, from: BlockNumber) -> ZkSyncDeployedContract<T> {
        let abi = zksync_contract();
        ZkSyncDeployedContract {
            web3_contract: web3::contract::Contract::new(eth, address, abi.clone()),
            abi,
            version: ZkSyncContractVersion::V4,
            from,
            to: BlockNumber::Latest,
        }
    }
}

impl ZkSyncContractVersion {
    pub fn rollup_ops_blocks_from_bytes(
        &self,
        data: Vec<u8>,
    ) -> anyhow::Result<Vec<RollupOpsBlock>> {
        use ZkSyncContractVersion::*;
        let res = match self {
            V0 | V1 | V2 | V3 => vec![rollup_ops_blocks_from_bytes(data)?],
            V4 => rollup_ops_blocks_from_bytes_v4(data)?,
        };
        Ok(res)
    }
}

pub fn rollup_ops_blocks_from_bytes(input_data: Vec<u8>) -> Result<RollupOpsBlock, anyhow::Error> {
    let block_number_argument_id = 0;
    let fee_account_argument_id = 1;
    let public_data_argument_id = 3;
    let decoded_commitment_parameters = ethabi::decode(
        vec![
            ParamType::Uint(32),                                   // uint32 _blockNumber,
            ParamType::Uint(32),                                   // uint32 _feeAccount,
            ParamType::Array(Box::new(ParamType::FixedBytes(32))), // bytes32[] _newRoots,
            ParamType::Bytes,                                      // bytes calldata _publicData,
            ParamType::Bytes,                                      // bytes calldata _ethWitness,
            ParamType::Array(Box::new(ParamType::Uint(32))), // uint32[] calldata _ethWitnessSizes
        ]
        .as_slice(),
        input_data.as_slice(),
    )
    .map_err(|_| {
        anyhow::Error::from(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded parameters from commitment transaction",
        )))
    })?;

    if let (
        ethabi::Token::Uint(block_num),
        ethabi::Token::Uint(fee_acc),
        ethabi::Token::Bytes(public_data),
    ) = (
        &decoded_commitment_parameters[block_number_argument_id],
        &decoded_commitment_parameters[fee_account_argument_id],
        &decoded_commitment_parameters[public_data_argument_id],
    ) {
        let ops = get_rollup_ops_from_data(public_data.as_slice())?;
        let fee_account = fee_acc.as_u32();

        let block = RollupOpsBlock {
            block_num: block_num.as_u32(),
            ops,
            fee_account,
        };
        Ok(block)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't parse commitment parameters",
        )
        .into())
    }
}

pub fn get_rollup_ops_from_data(data: &[u8]) -> Result<Vec<ZkSyncOp>, anyhow::Error> {
    let mut current_pointer = 0;
    let mut ops = vec![];
    while current_pointer < data.len() {
        let op_type: u8 = data[current_pointer];

        let pub_data_size = ZkSyncOp::public_data_length(op_type)?;

        let pre = current_pointer;
        let post = pre + pub_data_size;

        let op = ZkSyncOp::from_public_data(&data[pre..post])?;

        ops.push(op);
        current_pointer += pub_data_size;
    }
    Ok(ops)
}

fn decode_commitment_parameters_v4(input_data: Vec<u8>) -> anyhow::Result<Vec<Token>> {
    let commit_operation = ParamType::Tuple(vec![
        Box::new(ParamType::Uint(32)),       // uint32 _blockNumber,
        Box::new(ParamType::Uint(32)),       // uint32 _feeAccount,
        Box::new(ParamType::FixedBytes(32)), // bytes32 encoded_root,
        Box::new(ParamType::Bytes),          // bytes calldata _publicData,
        Box::new(ParamType::Uint(32)),       // uint64 _timestamp,
        Box::new(ParamType::Array(Box::new(ParamType::Tuple(vec![
            Box::new(ParamType::Uint(32)), //uint32 public_data_offset
            Box::new(ParamType::Bytes),    // bytes eht_witness
        ])))), // uint32[] calldata onchainOps
    ]);
    let stored_block = ParamType::Tuple(vec![
        Box::new(ParamType::Uint(32)),       // uint32 _block_number
        Box::new(ParamType::Uint(32)),       // uint32 _number_of_processed_prior_ops
        Box::new(ParamType::FixedBytes(32)), //bytes32  processable_ops_hash
        Box::new(ParamType::Uint(32)),       // uint256 timestamp
        Box::new(ParamType::FixedBytes(32)), // bytes32 eth_encoded_root
        Box::new(ParamType::FixedBytes(32)), // commitment
    ]);
    ethabi::decode(
        vec![stored_block, ParamType::Array(Box::new(commit_operation))].as_slice(),
        input_data.as_slice(),
    )
    .map_err(|_| {
        anyhow::Error::from(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded parameters from commitment transaction",
        )))
    })
}
pub fn rollup_ops_blocks_from_bytes_v4(data: Vec<u8>) -> anyhow::Result<Vec<RollupOpsBlock>> {
    let fee_account_argument_id = 1;
    let public_data_argument_id = 3;

    let decoded_commitment_parameters = decode_commitment_parameters_v4(data)?;
    assert_eq!(decoded_commitment_parameters.len(), 2);

    if let (ethabi::Token::Tuple(block), ethabi::Token::Array(operations)) = (
        &decoded_commitment_parameters[0],
        &decoded_commitment_parameters[1],
    ) {
        let mut blocks = vec![];
        if let ethabi::Token::Uint(block_num) = block[0] {
            for operation in operations {
                if let ethabi::Token::Tuple(operation) = operation {
                    if let (ethabi::Token::Uint(fee_acc), ethabi::Token::Bytes(public_data)) = (
                        &operation[fee_account_argument_id],
                        &operation[public_data_argument_id],
                    ) {
                        let ops = get_rollup_ops_from_data(public_data.as_slice())?;
                        blocks.push(RollupOpsBlock {
                            block_num: block_num.as_u32(),
                            ops,
                            fee_account: fee_acc.as_u32(),
                        })
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "can't parse operation parameters",
                        )
                        .into());
                    }
                }
            }
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "can't parse block parameters",
            )
            .into());
        }
        Ok(blocks)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't parse commitment parameters",
        )
        .into())
    }
}
