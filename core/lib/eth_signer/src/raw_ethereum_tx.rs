//! This file is a copy-paste from https://github.com/tomusdrw/rust-web3/blob/master/src/api/accounts.rs#L39
//! We want to use our own Signer, which is independent of Transaction Sender.
//! Unfortunately, it is impossible to use public interfaces from web3 library.
//! The only thing which has been changed is optional parameters, which are necessary for signing transactions.
//! In the library, they are filling using eth_node.
//!
//! I see no big difference between transaction and transaction parameters.
//! We can refactor this code and adapt it for our needs better, but I prefer to reuse as much code as we can.
//! In the case where it will be possible to use only the web3 library without copy-paste, the changes will be small and simple
//! Link to @Deniallugo's PR to web3: https://github.com/tomusdrw/rust-web3/pull/630
use rlp::RlpStream;
use web3::{
    signing::{self, Signature},
    types::{AccessList, Address, SignedTransaction, U256, U64},
};

const LEGACY_TX_ID: u64 = 0;
const ACCESSLISTS_TX_ID: u64 = 1;
const EIP1559_TX_ID: u64 = 2;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RawTransaction {
    /// Transaction nonce
    pub nonce: U256,
    /// To address
    pub to: Option<Address>,
    /// Supplied gas
    pub gas: U256,
    /// Gas price (None for estimated gas price)
    pub gas_price: U256,
    /// Transferred value
    pub value: U256,
    /// Data
    pub data: Vec<u8>,
    /// The chain ID
    pub chain_id: u64,
    /// Transaction type, Some(1) for AccessList transaction, None for Legacy
    pub transaction_type: Option<U64>,
    /// Access list
    pub access_list: Option<AccessList>,
    /// Max fee per gas
    pub max_fee_per_gas: Option<U256>,
    /// miner bribe
    pub max_priority_fee_per_gas: Option<U256>,
}

/// A transaction used for RLP encoding, hashing and signing.
#[derive(Debug)]
pub struct Transaction {
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas: U256,
    pub gas_price: U256,
    pub value: U256,
    pub data: Vec<u8>,
    pub transaction_type: Option<U64>,
    pub access_list: AccessList,
    pub max_priority_fee_per_gas: U256,
}

impl Transaction {
    fn rlp_append_legacy(&self, stream: &mut RlpStream) {
        stream.append(&self.nonce);
        stream.append(&self.gas_price);
        stream.append(&self.gas);
        if let Some(to) = self.to {
            stream.append(&to);
        } else {
            stream.append(&"");
        }
        stream.append(&self.value);
        stream.append(&self.data);
    }

    fn encode_legacy(&self, chain_id: u64, signature: Option<&Signature>) -> RlpStream {
        let mut stream = RlpStream::new();
        stream.begin_list(9);

        self.rlp_append_legacy(&mut stream);

        if let Some(signature) = signature {
            self.rlp_append_signature(&mut stream, signature);
        } else {
            stream.append(&chain_id);
            stream.append(&0u8);
            stream.append(&0u8);
        }

        stream
    }

    fn encode_access_list_payload(
        &self,
        chain_id: u64,
        signature: Option<&Signature>,
    ) -> RlpStream {
        let mut stream = RlpStream::new();

        let list_size = if signature.is_some() { 11 } else { 8 };
        stream.begin_list(list_size);

        // append chain_id. from EIP-2930: chainId is defined to be an integer of arbitrary size.
        stream.append(&chain_id);

        self.rlp_append_legacy(&mut stream);
        self.rlp_append_access_list(&mut stream);

        if let Some(signature) = signature {
            self.rlp_append_signature(&mut stream, signature);
        }

        stream
    }

    fn encode_eip1559_payload(&self, chain_id: u64, signature: Option<&Signature>) -> RlpStream {
        let mut stream = RlpStream::new();

        let list_size = if signature.is_some() { 12 } else { 9 };
        stream.begin_list(list_size);

        // append chain_id. from EIP-2930: chainId is defined to be an integer of arbitrary size.
        stream.append(&chain_id);

        stream.append(&self.nonce);
        stream.append(&self.max_priority_fee_per_gas);
        stream.append(&self.gas_price);
        stream.append(&self.gas);
        if let Some(to) = self.to {
            stream.append(&to);
        } else {
            stream.append(&"");
        }
        stream.append(&self.value);
        stream.append(&self.data);

        self.rlp_append_access_list(&mut stream);

        if let Some(signature) = signature {
            self.rlp_append_signature(&mut stream, signature);
        }

        stream
    }

    fn rlp_append_signature(&self, stream: &mut RlpStream, signature: &Signature) {
        stream.append(&signature.v);
        stream.append(&U256::from_big_endian(signature.r.as_bytes()));
        stream.append(&U256::from_big_endian(signature.s.as_bytes()));
    }

    fn rlp_append_access_list(&self, stream: &mut RlpStream) {
        stream.begin_list(self.access_list.len());
        for access in self.access_list.iter() {
            stream.begin_list(2);
            stream.append(&access.address);
            stream.begin_list(access.storage_keys.len());
            for storage_key in access.storage_keys.iter() {
                stream.append(storage_key);
            }
        }
    }

    fn encode(&self, chain_id: u64, signature: Option<&Signature>) -> Vec<u8> {
        match self.transaction_type.map(|t| t.as_u64()) {
            Some(LEGACY_TX_ID) | None => {
                let stream = self.encode_legacy(chain_id, signature);
                stream.out().to_vec()
            }

            Some(ACCESSLISTS_TX_ID) => {
                let tx_id: u8 = ACCESSLISTS_TX_ID as u8;
                let stream = self.encode_access_list_payload(chain_id, signature);
                [&[tx_id], stream.as_raw()].concat()
            }

            Some(EIP1559_TX_ID) => {
                let tx_id: u8 = EIP1559_TX_ID as u8;
                let stream = self.encode_eip1559_payload(chain_id, signature);
                [&[tx_id], stream.as_raw()].concat()
            }

            _ => {
                panic!("Unsupported transaction type");
            }
        }
    }

    /// Sign and return a raw signed transaction.
    pub fn sign(self, sign: impl signing::Key, chain_id: u64) -> SignedTransaction {
        let adjust_v_value = matches!(
            self.transaction_type.map(|t| t.as_u64()),
            Some(LEGACY_TX_ID) | None
        );

        let encoded = self.encode(chain_id, None);

        let hash = signing::keccak256(encoded.as_ref());

        let signature = if adjust_v_value {
            sign.sign(&hash, Some(chain_id))
                .expect("hash is non-zero 32-bytes; qed")
        } else {
            sign.sign_message(&hash)
                .expect("hash is non-zero 32-bytes; qed")
        };

        let signed = self.encode(chain_id, Some(&signature));
        let transaction_hash = signing::keccak256(signed.as_ref()).into();

        SignedTransaction {
            message_hash: hash.into(),
            v: signature.v,
            r: signature.r,
            s: signature.s,
            raw_transaction: signed.into(),
            transaction_hash,
        }
    }
}
