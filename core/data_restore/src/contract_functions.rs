use crate::data_restore_driver::ForkType;
use crate::eth_tx_helpers::get_input_data_from_ethereum_transaction;
use ethabi::ParamType;
use models::node::account::Account;
use models::params::{INPUT_DATA_ADDRESS_BYTES_WIDTH, INPUT_DATA_ROOT_HASH_BYTES_WIDTH};
use web3::contract::{Contract, Options};
use web3::futures::Future;
use web3::types::{Address, BlockNumber, Transaction, U256};
use web3::Transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParametersOfGenesisTx {
    /// constructor(
    ///    Governance _govTarget, Verifier _verifierTarget, ZkSync _zkSyncTarget,
    ///    bytes32 _genesisRoot, address _firstValidator, address _governor,
    ///    address _feeAccountAddress
    /// )
    Initial,
    /// constructor(
    ///    Governance _govTarget, Verifier _verifierTarget, ZkSync _zkSyncTarget, BlockProcessor _blockProcessor,
    ///    bytes32 _genesisRoot, address _firstValidator, address _governor,
    ///    address _feeAccountAddress
    /// )
    BlockProcessorAdded,
}

impl ParametersOfGenesisTx {
    pub fn from_fork_type(fork_type: ForkType) -> Self {
        match fork_type {
            ForkType::Initial => Self::Initial,
            ForkType::BlockProcessorAdded => Self::BlockProcessorAdded,
        }
    }

    pub fn get_parameters(self) -> Vec<ParamType> {
        let mut res = vec![];
        res.push(ParamType::Address); // Governance contract address
        res.push(ParamType::Address); // Verifier contract address
        res.push(ParamType::Address); // zkSync contract address
        res.push(ParamType::FixedBytes(INPUT_DATA_ROOT_HASH_BYTES_WIDTH)); // Genesis root
        res.push(ParamType::Address); // First validator (committer) address
        res.push(ParamType::Address); // Governor address
        res.push(ParamType::Address); // Fee account address
        if self == Self::BlockProcessorAdded {
            res.push(ParamType::Address); // Block processor address
        }
        res
    }

    pub fn get_init_parameters_width(self) -> usize {
        match self {
            Self::Initial => 6 * INPUT_DATA_ADDRESS_BYTES_WIDTH + INPUT_DATA_ROOT_HASH_BYTES_WIDTH,
            Self::BlockProcessorAdded => {
                7 * INPUT_DATA_ADDRESS_BYTES_WIDTH + INPUT_DATA_ROOT_HASH_BYTES_WIDTH
            }
        }
    }

    pub fn get_fee_account_address_argument_id(self) -> usize {
        match self {
            Self::Initial => 6,
            Self::BlockProcessorAdded => 7,
        }
    }
}

fn get_genesis_account_with_parameters(
    genesis_transaction: &Transaction,
    parameters: &[ParamType],
    encoded_init_parameters_width: usize,
    fee_account_address_argument_id: usize,
) -> Result<Account, failure::Error> {
    let input_data = get_input_data_from_ethereum_transaction(&genesis_transaction)?;

    // Input for contract constructor contains the bytecode of the contract and
    // encoded arguments after it.
    // We are not interested in the bytecode and we know the size of arguments,
    // so we can simply cut the parameters bytes from the end of input array,
    // and then decode them to access required data.
    let encoded_init_parameters =
        input_data[input_data.len() - encoded_init_parameters_width..].to_vec();

    let decoded_init_parameters = ethabi::decode(parameters, encoded_init_parameters.as_slice())
        .map_err(|_| {
            failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
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
    .ok_or_else(|| Err("Invalid token in parameters"))
    .map_err(|_: Result<Account, _>| {
        failure::Error::from_boxed_compat(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "can't get decoded init parameter from contract creation transaction",
        )))
    })
}

/// Returns Rollup genesis (fees) account from the input of the Rollup contract creation transaction
///
/// # Arguments
///
/// * `transaction` - Ethereum Rollup contract creation transaction description
/// * `genesis_tx_signature` - genesis tx signature
///
pub fn get_genesis_account(
    genesis_transaction: &Transaction,
    genesis_tx_signature: ParametersOfGenesisTx,
) -> Result<Account, failure::Error> {
    get_genesis_account_with_parameters(
        genesis_transaction,
        &genesis_tx_signature.get_parameters(),
        genesis_tx_signature.get_init_parameters_width(),
        genesis_tx_signature.get_fee_account_address_argument_id(),
    )
}

/// Returns total number of verified blocks on Rollup contract
///
/// # Arguments
///
/// * `web3` - Web3 provider url
/// * `franklin_contract` - Rollup contract
///
pub fn get_total_verified_blocks<T: Transport>(
    franklin_contract: &(ethabi::Contract, Contract<T>),
) -> u32 {
    franklin_contract
        .1
        .query::<U256, Option<Address>, Option<BlockNumber>, ()>(
            "totalBlocksVerified",
            (),
            None,
            Options::default(),
            None,
        )
        .wait()
        .unwrap()
        .as_u32()
}
