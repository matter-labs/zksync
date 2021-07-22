use parity_crypto::{publickey::Signature, Keccak256};
use rlp::RlpStream;
use serde::{Deserialize, Serialize};
use zksync_types::{H160, U256};

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

fn find_first_nonzero(vector: &[u8]) -> usize {
    let mut result: usize = 0;
    for el in vector {
        if *el == 0 {
            result += 1;
        } else {
            break;
        }
    }

    result
}

impl RawTransaction {
    pub fn rlp_encode_tx(&self, sig: Signature) -> Vec<u8> {
        let signature = to_ecdsa(sig, self.chain_id);
        let mut tx = RlpStream::new();
        tx.begin_unbounded_list();
        self.encode(&mut tx);
        tx.append(&signature.v);
        let r_start = find_first_nonzero(&signature.r);
        let r = &signature.r[r_start..];
        tx.append(&r);
        let s_start = find_first_nonzero(&signature.s);
        let s = &signature.s[s_start..];
        tx.append(&s);
        tx.finalize_unbounded_list();
        tx.out()
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hash = RlpStream::new();
        hash.begin_unbounded_list();
        self.encode(&mut hash);
        hash.append(&vec![self.chain_id]);
        hash.append(&U256::zero());
        hash.append(&U256::zero());
        hash.finalize_unbounded_list();
        hash.out().keccak256()
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

fn to_ecdsa(sig: Signature, chain_id: u8) -> EcdsaSig {
    EcdsaSig {
        v: vec![sig.v() as u8 + chain_id * 2 + 35],
        r: sig.r().to_vec(),
        s: sig.s().to_vec(),
    }
}

pub struct EcdsaSig {
    v: Vec<u8>,
    r: Vec<u8>,
    s: Vec<u8>,
}
