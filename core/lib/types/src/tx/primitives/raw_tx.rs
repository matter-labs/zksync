use crate::{H160, U256};
use rlp::RlpStream;
use serde::{Deserialize, Serialize};

/// Description of a Transaction, pending or in the chain.
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct RawTransaction {
    /// Chain id: mainnet => 1, rinkeby => 4, ropsten => 43, etc.
    pub chain_id: u8,
    /// Nonce
    pub nonce: U256,
    /// Recipient (None when contract creation)
    pub to: Option<H160>,
    /// Transfered value
    pub value: U256,
    /// Gas Price
    #[serde(rename = "gasPrice")]
    pub gas_price: U256,
    /// Gas amount
    pub gas: U256,
    /// Input data
    pub data: Vec<u8>,
}

impl RawTransaction {
    pub fn tx_data(&self) -> Vec<u8> {
        let mut tx_data = RlpStream::new();
        tx_data.begin_unbounded_list();
        self.encode(&mut tx_data);
        tx_data.append(&vec![self.chain_id]);
        tx_data.append(&U256::zero());
        tx_data.append(&U256::zero());
        tx_data.finalize_unbounded_list();
        tx_data.out()
    }

    pub fn encode(&self, s: &mut RlpStream) {
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas);
        if let Some(ref t) = self.to {
            s.append(t);
        } else {
            s.append(&vec![]);
        }
        s.append(&self.value);
        s.append(&self.data);
    }
}
