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
use ff::{PrimeField, PrimeFieldRepr};
use sapling_crypto::circuit::float_point::convert_to_float;
use sapling_crypto::eddsa::Signature;
use sapling_crypto::jubjub::{edwards, FixedGenerators, JubjubEngine, Unknown};

use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use web3::types::Address;

// Signed by user.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pubkey {
    pub pk_x: Fr,
    pub pk_y: Fr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transfer {
    pub from: Pubkey,
    pub to: Pubkey,
    pub token: TokenId,
    pub amount: TokenAmount,
    pub fee: FeeAmount,
    pub nonce: Nonce,
    // TODO: Signature unimplemented
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    pub to: Pubkey,
    pub token: TokenId,
    pub amount: TokenAmount,
    pub fee: FeeAmount,
    pub nonce: Nonce,
    // TODO: Signature unimplemented
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Withdraw {
    pub account: Pubkey,
    pub eth_address: Address,
    pub token: TokenId,
    /// None -> withdraw all
    pub amount: TokenAmount,
    pub fee: FeeAmount,
    pub nonce: Nonce,
    // TODO: Signature unimplemented
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Close {
    pub account: Pubkey,
    pub nonce: Nonce,
    // TODO: Signature unimplemented
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinTx {
    Transfer(Transfer),
    Deposit(Deposit),
    Withdraw(Withdraw),
    Close(Close),
}

impl FranklinTx {
    pub fn hash(&self) -> Vec<u8> {
        unimplemented!("hash")
    }
}

//
//impl DepositTx {
//    fn get_bytes(&self) -> Vec<u8> {
//        let mut out = Vec::new();
//        out.extend_from_slice(&self.token.to_be_bytes());
//        out.extend_from_slice(&self.amount.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.fee.to_be_bytes());
//        self.pub_x.into_repr().write_be(&mut out).unwrap();
//        self.pub_y.into_repr().write_be(&mut out).unwrap();
//        out.extend_from_slice(&self.nonce.to_be_bytes());
//        out.extend_from_slice(&self.good_until_block.to_be_bytes());
//        out
//    }
//}
//
//
//impl TransferToNewTx {
//    fn get_bytes(&self) -> Vec<u8> {
//        let mut out = Vec::new();
//        out.extend_from_slice(&self.from.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.token.to_be_bytes());
//        out.extend_from_slice(&self.amount.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.fee.to_be_bytes());
//        self.pub_x.into_repr().write_be(&mut out).unwrap();
//        self.pub_y.into_repr().write_be(&mut out).unwrap();
//        out.extend_from_slice(&self.nonce.to_be_bytes());
//        out.extend_from_slice(&self.good_until_block.to_be_bytes());
//        out
//    }
//}
//
//
//impl PartialExitTx {
//    fn get_bytes(&self) -> Vec<u8> {
//        let mut out = Vec::new();
//        out.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.token.to_be_bytes());
//        out.extend_from_slice(&self.amount.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.fee.to_be_bytes());
//        out.extend_from_slice(&self.eth_address);
//        out.extend_from_slice(&self.nonce.to_be_bytes());
//        out.extend_from_slice(&self.good_until_block.to_be_bytes());
//        out
//    }
//}
//
//
//impl TransferTx {
//    fn get_bytes(&self) -> Vec<u8> {
//        let mut out = Vec::new();
//        out.extend_from_slice(&self.from.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.to.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.token.to_be_bytes());
//        out.extend_from_slice(&self.amount.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.fee.to_be_bytes());
//        out.extend_from_slice(&self.nonce.to_be_bytes());
//        out.extend_from_slice(&self.good_until_block.to_be_bytes());
//        out
//    }
//}
//
//impl CloseTx {
//    fn get_bytes(&self) -> Vec<u8> {
//        let mut out = Vec::new();
//        out.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
//        out.extend_from_slice(&self.nonce.to_be_bytes());
//        out.extend_from_slice(&self.good_until_block.to_be_bytes());
//        out
//    }
//}

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
