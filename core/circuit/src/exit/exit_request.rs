use ff::{BitIterator, PrimeField};
use franklin_crypto::jubjub::JubjubEngine;
use models::plasma::params as plasma_constants;

// This is an exit request

#[derive(Clone)]
pub struct ExitRequest<E: JubjubEngine> {
    pub from: Option<E::Fr>,
    // keep an amount in request for ease of public data serialization
    // it's NOT USED in a zkSNARK
    pub amount: Option<E::Fr>,
}

impl<E: JubjubEngine> ExitRequest<E> {
    pub fn public_data_into_bits(&self) -> Vec<bool> {
        // fields are
        // - from
        // - amount
        // - compressed public key
        let mut from: Vec<bool> = BitIterator::new(self.from.unwrap().into_repr()).collect();
        from.reverse();
        from.truncate(plasma_constants::BALANCE_TREE_DEPTH);
        // reverse again to have BE as in Ethereum native types
        from.reverse();

        let mut amount: Vec<bool> = BitIterator::new(self.amount.unwrap().into_repr()).collect();
        amount.reverse();
        amount.truncate(plasma_constants::BALANCE_BIT_WIDTH);
        // reverse again to have BE as in Ethereum native types
        amount.reverse();

        let mut packed: Vec<bool> = vec![];
        packed.extend(from.into_iter());
        packed.extend(amount.into_iter());

        packed
    }

    pub fn data_as_bytes(&self) -> Vec<u8> {
        let raw_data: Vec<bool> = self.public_data_into_bits();

        let mut message_bytes: Vec<u8> = vec![];

        let byte_chunks = raw_data.chunks(8);
        for byte_chunk in byte_chunks {
            let mut byte = 0u8;
            for (i, bit) in byte_chunk.iter().enumerate() {
                if *bit {
                    byte |= 1 << i;
                }
            }
            message_bytes.push(byte);
        }

        message_bytes
    }
}
