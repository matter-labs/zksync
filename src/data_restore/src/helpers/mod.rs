use web3::types::H256;
use tiny_keccak::keccak256;

pub type ABI = (&'static [u8], &'static str);
pub const FRANKLIN_MAINNET_ADDRESS: &'static str = "fddb8167fef957f7cc72686094fac1d31be5ecfe";
pub const FRANKLIN_RINKEBY_ADDRESS: &'static str = "fddb8167fef957f7cc72686094fac1d31be5ecfe";
pub const INFURA_MAINNET_ENDPOINT: &'static str = "https://mainnet.infura.io/";
pub const INFURA_RINKEBY_ENDPOINT: &'static str = "https://rinkeby.infura.io/";
pub const PLASMA_RINKEBY_ABI: ABI = (
    include_bytes!("../../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.abi"),
    include_str!("../../../../contracts/bin/contracts_PlasmaTester_sol_PlasmaTester.bin"),
);
pub const PLASMA_MAINNET_ABI: ABI = (
    include_bytes!("../../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.abi"),
    include_str!("../../../../contracts/bin/contracts_PlasmaContract_sol_PlasmaContract.bin"),
);

#[derive(Debug, Copy, Clone)]
pub enum InfuraEndpoint {
    Mainnet,
    Rinkeby
}

pub fn keccak256_hash(bytes: &[u8]) -> Vec<u8> {
    keccak256(bytes).into_iter().cloned().collect()
}

pub fn get_topic_keccak_hash(topic: &str) -> web3::types::H256 {
    let topic_data: Vec<u8> = From::from(topic);
    let topic_data_vec: &[u8] = topic_data.as_slice();
    let topic_keccak_data: Vec<u8> = keccak256_hash(topic_data_vec);
    let topic_keccak_data_vec: &[u8] = topic_keccak_data.as_slice();
    let topic_h256 = H256::from_slice(topic_keccak_data_vec);
    topic_h256
}

#[derive(Debug)]
pub enum DataRestoreError {
    Unknown(String),
    WrongType,
    NoData(String),
    NonexistentAccount,
    WrongAmount,
    WrongEndpoint,
    WrongPubKey,
    DoubleExit,
    StateUpdate(String),
}

impl std::string::ToString for DataRestoreError {
    fn to_string(&self) -> String {
        match self {
            DataRestoreError::Unknown(text)      => format!("Unknown {}", text),
            DataRestoreError::WrongType          => "Wrong type".to_owned(),
            DataRestoreError::NoData(text)       => format!("No data {}", text),
            DataRestoreError::NonexistentAccount => "Nonexistent account".to_owned(),
            DataRestoreError::WrongAmount        => "Wrong amount".to_owned(),
            DataRestoreError::WrongEndpoint      => "Wrong endpoint".to_owned(),
            DataRestoreError::WrongPubKey        => "Wrong pubkey".to_owned(),
            DataRestoreError::DoubleExit         => "Double exit".to_owned(),
            DataRestoreError::StateUpdate(text)  => format!("Error during state update {}", text),
        }
    }
}
