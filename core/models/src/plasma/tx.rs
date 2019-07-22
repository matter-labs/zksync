use super::{params, AccountId, BlockNumber, FeeAmount, Nonce, TokenAmount, TokenId};
use super::{Engine, Fr};
use super::{PrivateKey, PublicKey};
use crate::plasma::circuit::sig::TransactionSignature;
use crate::plasma::circuit::transfer::Tx;
use crate::plasma::circuit::utils::{
    encode_fr_into_fs, encode_fs_into_fr, le_bit_vector_into_field_element,
};
use crate::primitives::{get_bits_le_fixed_u128, pack_bits_into_bytes};
use bigdecimal::{BigDecimal, ToPrimitive};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use ff::{PrimeField, PrimeFieldRepr};
use sapling_crypto::circuit::float_point::convert_to_float;
use sapling_crypto::eddsa::Signature;
use sapling_crypto::jubjub::{edwards, FixedGenerators, JubjubEngine, Unknown};

use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use web3::types::Address;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DepositTx {
    pub token: TokenId,
    pub amount: TokenAmount,
    pub fee: FeeAmount,
    pub pub_x: Fr,
    pub pub_y: Fr,
    pub nonce: Nonce,
    pub good_until_block: BlockNumber,
}

impl DepositTx {
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&self.amount.to_be_bytes()[1..]);
        out.extend_from_slice(&self.fee.to_be_bytes());
        self.pub_x.into_repr().write_be(&mut out).unwrap();
        self.pub_y.into_repr().write_be(&mut out).unwrap();
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.good_until_block.to_be_bytes());
        out
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TransferToNewTx {
    pub from: AccountId,
    pub token: TokenId,
    pub amount: TokenAmount,
    pub fee: FeeAmount,
    pub pub_x: Fr,
    pub pub_y: Fr,
    pub nonce: Nonce,
    pub good_until_block: BlockNumber,
}

impl TransferToNewTx {
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.from.to_be_bytes()[1..]);
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&self.amount.to_be_bytes()[1..]);
        out.extend_from_slice(&self.fee.to_be_bytes());
        self.pub_x.into_repr().write_be(&mut out).unwrap();
        self.pub_y.into_repr().write_be(&mut out).unwrap();
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.good_until_block.to_be_bytes());
        out
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PartialExitTx {
    pub account_id: AccountId,
    pub token: TokenId,
    pub amount: TokenAmount,
    pub fee: FeeAmount,
    pub eth_address: Address,
    pub nonce: Nonce,
    pub good_until_block: BlockNumber,
}

impl PartialExitTx {
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&self.amount.to_be_bytes()[1..]);
        out.extend_from_slice(&self.fee.to_be_bytes());
        out.extend_from_slice(&self.eth_address);
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.good_until_block.to_be_bytes());
        out
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TransferTx {
    pub from: AccountId,
    pub to: AccountId,
    pub token: TokenId,
    pub amount: TokenAmount,
    pub fee: FeeAmount,
    pub nonce: Nonce,
    pub good_until_block: BlockNumber,
}

impl TransferTx {
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.from.to_be_bytes()[1..]);
        out.extend_from_slice(&self.to.to_be_bytes()[1..]);
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&self.amount.to_be_bytes()[1..]);
        out.extend_from_slice(&self.fee.to_be_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.good_until_block.to_be_bytes());
        out
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CloseTx {
    pub account_id: AccountId,
    pub nonce: Nonce,
    pub good_until_block: BlockNumber,
}

impl CloseTx {
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.good_until_block.to_be_bytes());
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinTx {
    Deposit(DepositTx),
    TransferToNew(TransferToNewTx),
    PartialExit(PartialExitTx),
    Close(CloseTx),
    Transfer(TransferTx),
}

impl FranklinTx {
    pub fn chunks(&self) -> usize {
        match self {
            FranklinTx::Deposit(_) => 5,
            FranklinTx::TransferToNew(_) => 5,
            FranklinTx::PartialExit(_) => 4,
            FranklinTx::Close(_) => 1,
            FranklinTx::Transfer(_) => 2,
        }
    }
}

//impl UncheckedSignedTx {
//    pub fn verify_signature(&self, pub_key: &PublicKey) -> Result<CheckedSignedTx, ()> {
//        let serialized_message = self.tx_data.get_bytes();
//        unimplemented!()
////        if let Ok(signature) = self.signature.to_jubjub_eddsa() {
////            let p_g = FixedGenerators::SpendingKeyGenerator;
////            let valid = public_key.verify_for_raw_message(
////                &as_bytes,
////                &signature,
////                p_g,
////                &params::JUBJUB_PARAMS,
////                30,
////            );
////            valid;
////        } else {
////            false
////        }
//    }
//}

impl FranklinTx {
    fn get_bytes(&self) -> Vec<u8> {
        match self {
            FranklinTx::Deposit(tx) => tx.get_bytes(),
            FranklinTx::TransferToNew(tx) => tx.get_bytes(),
            FranklinTx::PartialExit(tx) => tx.get_bytes(),
            FranklinTx::Close(tx) => tx.get_bytes(),
            FranklinTx::Transfer(tx) => tx.get_bytes(),
        }
    }

    pub fn hash(&self) -> Vec<u8> {
        // TODO: maybe use other hash?
        let mut hasher = Sha256::new();
        hasher.input(&self.get_bytes());
        let mut out = vec![0u8; 32];
        hasher.result(&mut out);
        out
    }

    pub fn nonce(&self) -> Nonce {
        match self {
            FranklinTx::Deposit(tx) => tx.nonce,
            FranklinTx::TransferToNew(tx) => tx.nonce,
            FranklinTx::PartialExit(tx) => tx.nonce,
            FranklinTx::Close(tx) => tx.nonce,
            FranklinTx::Transfer(tx) => tx.nonce,
        }
    }

    pub fn account_id(&self) -> Option<AccountId> {
        match self {
            FranklinTx::Deposit(tx) => None,
            FranklinTx::TransferToNew(tx) => Some(tx.from),
            FranklinTx::PartialExit(tx) => Some(tx.account_id),
            FranklinTx::Close(tx) => Some(tx.account_id),
            FranklinTx::Transfer(tx) => Some(tx.from),
        }
    }
}

//impl TransferTx {
//    pub fn create_signed_tx(
//        from: u32,
//        to: u32,
//        amount: BigDecimal,
//        fee: BigDecimal,
//        nonce: u32,
//        good_until_block: u32,
//        private_key: &PrivateKey,
//    ) -> Self {
//        let tx = TransferTx {
//            from,
//            to,
//            token: 0,
//            amount: amount.clone(),
//            fee: fee.clone(),
//            nonce,
//            good_until_block,
//            signature: TxSignature::default(),
//        };
//
//        let message_bits = tx.message_bits();
//        let as_bytes = pack_bits_into_bytes(message_bits);
//
//        let rng = &mut rand::thread_rng();
//        let p_g = FixedGenerators::SpendingKeyGenerator;
//
//        let signature = TxSignature::from(private_key.sign_raw_message(
//            &as_bytes,
//            rng,
//            p_g,
//            &params::JUBJUB_PARAMS,
//            as_bytes.len(),
//        ));
//
//        TransferTx {
//            from,
//            to,
//            token: 0,
//            amount,
//            fee,
//            nonce,
//            good_until_block,
//            signature,
//        }
//    }
//
//    pub fn verify_sig(&self, public_key: &PublicKey) -> bool {
//        let message_bits = self.message_bits();
//        if message_bits.len() % 8 != 0 {
//            error!("Invalid message length");
//            return false;
//        }
//        let as_bytes = pack_bits_into_bytes(message_bits);
//        //use rustc_hex::ToHex;
//        //let hex: String = as_bytes.clone().to_hex();
//        //debug!("Transaction bytes = {}", hex);
//        if let Ok(signature) = self.signature.to_jubjub_eddsa() {
//            //debug!("Successfuly converted to eddsa signature");
//            let p_g = FixedGenerators::SpendingKeyGenerator;
//            let valid = public_key.verify_for_raw_message(
//                &as_bytes,
//                &signature,
//                p_g,
//                &params::JUBJUB_PARAMS,
//                30,
//            );
//
//            return valid;
//        }
//        //debug!("Signature was not deserialized");
//
//        false
//    }
//
//    pub fn validate(&self) -> Result<(), String> {
//        use bigdecimal::Zero;
//        if self.from == self.to {
//            return Err(format!("tx.from may not equal tx.to: {}", self.from));
//        }
//        if self.amount == BigDecimal::zero() {
//            return Err("zero amount is not allowed".to_string());
//        }
//
//        Ok(())
//    }
//}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TxSignature {
    pub r_x: Fr,
    pub r_y: Fr,
    pub s: Fr,
}

impl TxSignature {
    pub fn try_from(signature: TransactionSignature<Engine>) -> Result<Self, String> {
        let (x, y) = signature.r.into_xy();

        Ok(Self {
            r_x: x,
            r_y: y,
            s: signature.s,
        })
    }

    pub fn from(signature: Signature<Engine>) -> Self {
        let (r_x, r_y) = signature.r.into_xy();
        let s = encode_fs_into_fr::<Engine>(signature.s);

        Self { r_x, r_y, s }
    }

    pub fn to_jubjub_eddsa(&self) -> Result<Signature<Engine>, String> {
        let r =
            edwards::Point::<Engine, Unknown>::from_xy(self.r_x, self.r_y, &params::JUBJUB_PARAMS)
                .expect("make point from X and Y");
        let s: <Engine as JubjubEngine>::Fs = encode_fr_into_fs::<Engine>(self.s);

        Ok(Signature::<Engine> { r, s })
    }
}
