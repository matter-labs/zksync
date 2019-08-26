use super::{Nonce, TokenId};
use crate::node::{pack_fee_amount, pack_token_amount};
use bigdecimal::BigDecimal;
use crypto::{digest::Digest, sha2::Sha256};

use super::account::AccountAddress;
use web3::types::Address;

/// Signed by user.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TxType {
    Transfer,
    Deposit,
    Withdraw,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transfer {
    // TODO: derrive account address from signature
    pub from: AccountAddress,
    pub to: AccountAddress,
    pub token: TokenId,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub nonce: Nonce,
    // TODO: Signature unimplemented
}

impl Transfer {
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.from.data);
        out.extend_from_slice(&self.to.data);
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&pack_token_amount(&self.amount));
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    // TODO: derrive account address from signature
    pub to: AccountAddress,
    pub token: TokenId,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub nonce: Nonce,
    // TODO: Signature unimplemented
}

impl Deposit {
    const TX_TYPE: u8 = 1;
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.to.data);
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&pack_token_amount(&self.amount));
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Withdraw {
    // TODO: derrive account address from signature
    pub account: AccountAddress,
    pub eth_address: Address,
    pub token: TokenId,
    /// None -> withdraw all
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub nonce: Nonce,
    // TODO: Signature unimplemented
}

impl Withdraw {
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.account.data);
        out.extend_from_slice(&self.eth_address);
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&pack_token_amount(&self.amount));
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Close {
    // TODO: derrive account address from signature
    pub account: AccountAddress,
    pub nonce: Nonce,
    // TODO: Signature unimplemented
}

impl Close {
    fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.account.data);
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }
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
        let bytes = match self {
            FranklinTx::Transfer(tx) => tx.get_bytes(),
            FranklinTx::Deposit(tx) => tx.get_bytes(),
            FranklinTx::Withdraw(tx) => tx.get_bytes(),
            FranklinTx::Close(tx) => tx.get_bytes(),
        };

        let mut hasher = Sha256::new();
        hasher.input(&bytes);
        let mut out = vec![0u8; 32];
        hasher.result(&mut out);
        out
    }

    pub fn account(&self) -> AccountAddress {
        match self {
            FranklinTx::Transfer(tx) => tx.from.clone(),
            FranklinTx::Deposit(tx) => tx.to.clone(),
            FranklinTx::Withdraw(tx) => tx.account.clone(),
            FranklinTx::Close(tx) => tx.account.clone(),
        }
    }

    pub fn nonce(&self) -> Nonce {
        match self {
            FranklinTx::Transfer(tx) => tx.nonce,
            FranklinTx::Deposit(tx) => tx.nonce,
            FranklinTx::Withdraw(tx) => tx.nonce,
            FranklinTx::Close(tx) => tx.nonce,
        }
    }

    pub fn min_number_of_chunks(&self) -> usize {
        // TODO use spec
        1
    }

    pub fn check_signature(&self) -> bool {
        true
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
//
//#[derive(Debug, Clone, Default, Serialize, Deserialize)]
//pub struct TxSignature {
//    pub r_x: Fr,
//    pub r_y: Fr,
//    pub s: Fr,
//}
//
//impl TxSignature {
//    pub fn try_from(signature: TransactionSignature<Engine>) -> Result<Self, String> {
//        let (x, y) = signature.r.into_xy();
//
//        Ok(Self {
//            r_x: x,
//            r_y: y,
//            s: signature.s,
//        })
//    }
//
//    pub fn from(signature: Signature<Engine>) -> Self {
//        let (r_x, r_y) = signature.r.into_xy();
//        let s = encode_fs_into_fr::<Engine>(signature.s);
//
//        Self { r_x, r_y, s }
//    }
//
//    pub fn to_jubjub_eddsa(&self) -> Result<Signature<Engine>, String> {
//        let r =
//            edwards::Point::<Engine, Unknown>::from_xy(self.r_x, self.r_y, &params::JUBJUB_PARAMS)
//                .expect("make point from X and Y");
//        let s: <Engine as JubjubEngine>::Fs = encode_fr_into_fs::<Engine>(self.s);
//
//        Ok(Signature::<Engine> { r, s })
//    }
//}
