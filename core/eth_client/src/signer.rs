use parity_crypto::publickey::sign;
use rlp::RlpStream;
use tiny_keccak::keccak256;
use web3::types::{H160, H256, U256};

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
    /// Signs and returns the RLP-encoded transaction
    pub fn sign(&self, private_key: &H256) -> Vec<u8> {
        let hash = self.hash();
        let sig = ecdsa_sign(hash, &private_key, self.chain_id);
        let mut tx = RlpStream::new();
        tx.begin_unbounded_list();
        self.encode(&mut tx);
        tx.append(&sig.v);
        let r_start = find_first_nonzero(&sig.r);
        let r = &sig.r.clone()[r_start..];
        tx.append(&r);
        let s_start = find_first_nonzero(&sig.s);
        let s = &sig.s[s_start..];
        tx.append(&s);
        tx.finalize_unbounded_list();
        tx.out()
    }

    fn hash(&self) -> [u8; 32] {
        let mut hash = RlpStream::new();
        hash.begin_unbounded_list();
        self.encode(&mut hash);
        hash.append(&vec![self.chain_id]);
        hash.append(&U256::zero());
        hash.append(&U256::zero());
        hash.finalize_unbounded_list();
        keccak256(&hash.out()).into()
    }

    fn encode(&self, s: &mut RlpStream) {
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

fn ecdsa_sign(hash: [u8; 32], private_key: &H256, chain_id: u8) -> EcdsaSig {
    let sig = sign(&(*private_key).into(), &hash.into()).expect("failed to sign eth message");

    //debug!("V m8 {:?}", v);

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
