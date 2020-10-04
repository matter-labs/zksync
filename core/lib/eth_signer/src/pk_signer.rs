use crate::SignerError;
use parity_crypto::{publickey::sign, Keccak256};
use rlp::RlpStream;
use types::tx::{PackedEthSignature, RawTransaction, TxEthSignature};
use types::{Address, H256};

#[derive(Clone)]
pub struct PrivateKeySigner {
    private_key: H256,
    address: Address,
}

impl PrivateKeySigner {
    pub fn new(private_key: H256) -> Self {
        // FIXME: remove unwrap add Result anf error code
        let address = PackedEthSignature::address_from_private_key(&private_key).unwrap();
        Self {
            private_key,
            address,
        }
    }

    pub fn address(&self) -> Address {
        self.address
    }

    /// FIXME:
    pub fn sign_message(&self, message: &[u8]) -> Result<TxEthSignature, SignerError> {
        let pack = PackedEthSignature::sign(&self.private_key, &message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(TxEthSignature::EthereumSignature(pack))
    }

    pub fn sign_transaction(&self, raw_tx: RawTransaction) -> Result<TxEthSignature, SignerError> {
        let hash = raw_tx.tx_data().keccak256();
        let signature = self
            .ecdsa_sign(hash, raw_tx.chain_id)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        let mut tx = RlpStream::new();
        tx.begin_unbounded_list();
        raw_tx.encode(&mut tx);
        tx.append(&signature.v);
        let r_start = find_first_nonzero(&signature.r);
        let r = &signature.r[r_start..];
        tx.append(&r);
        let s_start = find_first_nonzero(&signature.s);
        let s = &signature.s[s_start..];
        tx.append(&s);
        let packed_signature = PackedEthSignature::deserialize_packed(&tx.out())
            .map_err(|_| SignerError::SigningFailed("sda".to_string()))?;
        // FIXME:
        Ok(TxEthSignature::EthereumSignature(packed_signature))
    }

    fn ecdsa_sign(&self, hash: [u8; 32], chain_id: u8) -> Result<EcdsaSig, SignerError> {
        let packed_signature = sign(&self.private_key.into(), &hash.into())
            .map_err(|_| SignerError::SigningFailed("sda".to_string()))?;
        // FIXME:;
        let sig = packed_signature.signature();
        Ok(EcdsaSig {
            v: vec![sig.v() as u8 + chain_id * 2 + 35],
            r: sig.r().to_vec(),
            s: sig.s().to_vec(),
        })
    }
}

struct EcdsaSig {
    v: Vec<u8>,
    r: Vec<u8>,
    s: Vec<u8>,
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
