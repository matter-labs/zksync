use crate::error::SignerError;
use crate::EthereumSigner;
use models::tx::TxEthSignature;
use rlp::RlpStream;
use web3::types::{H160, U256};

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
    pub async fn sign(&self, ethereum_signer: &EthereumSigner) -> Result<Vec<u8>, SignerError> {
        let tx_data = self.tx_data();
        let sig = ecdsa_sign(tx_data, &ethereum_signer, self.chain_id).await?;
        let mut tx = RlpStream::new();
        tx.begin_unbounded_list();
        self.encode(&mut tx);
        tx.append(&sig.v);
        let r_start = find_first_nonzero(&sig.r);
        let r = &sig.r[r_start..];
        tx.append(&r);
        let s_start = find_first_nonzero(&sig.s);
        let s = &sig.s[s_start..];
        tx.append(&s);
        tx.finalize_unbounded_list();
        Ok(tx.out())
    }

    fn tx_data(&self) -> Vec<u8> {
        let mut tx_data = RlpStream::new();
        tx_data.begin_unbounded_list();
        self.encode(&mut tx_data);
        tx_data.append(&vec![self.chain_id]);
        tx_data.append(&U256::zero());
        tx_data.append(&U256::zero());
        tx_data.finalize_unbounded_list();
        tx_data.out()
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

async fn ecdsa_sign(
    message: Vec<u8>,
    ethereum_signer: &EthereumSigner,
    chain_id: u8,
) -> Result<EcdsaSig, SignerError> {
    let tx_eth_signature = ethereum_signer.sign(&message, false).await?;

    if let TxEthSignature::EthereumSignature(packed_signature) = tx_eth_signature {
        let sig = packed_signature.signature();
        Ok(EcdsaSig {
            v: vec![sig.v() as u8 + chain_id * 2 + 35],
            r: sig.r().to_vec(),
            s: sig.s().to_vec(),
        })
    } else {
        Err(SignerError::SigningFailed("TODO".to_string()))
    }
}

pub struct EcdsaSig {
    v: Vec<u8>,
    r: Vec<u8>,
    s: Vec<u8>,
}
