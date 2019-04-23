use web3::types::H256;
use tiny_keccak::keccak256;

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
