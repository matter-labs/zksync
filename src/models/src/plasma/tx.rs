use crate::plasma::circuit::deposit::DepositRequest;
use crate::plasma::circuit::exit::ExitRequest;
use crate::plasma::circuit::sig::TransactionSignature;
use crate::plasma::circuit::transfer::Tx;
use crate::plasma::circuit::utils::{
    encode_fr_into_fs, encode_fs_into_fr, le_bit_vector_into_field_element,
};
use crate::plasma::params;
use crate::plasma::{Engine, Fr};
use crate::plasma::{PrivateKey, PublicKey};
use crate::primitives::{get_bits_le_fixed_u128, pack_bits_into_bytes};
use bigdecimal::{BigDecimal, ToPrimitive};
use ff::PrimeField;
use sapling_crypto::circuit::float_point::convert_to_float;
use sapling_crypto::eddsa::Signature;
use sapling_crypto::jubjub::{edwards, FixedGenerators, JubjubEngine, Unknown};

use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};

pub const TRANSFER_TX: &'static str = "Transfer";
pub const DEPOSIT_TX: &'static str = "Deposit";
pub const EXIT_TX: &'static str = "Exit";

#[derive(Clone)]
pub enum TransactionType {
    Transfer { tx: TransferTx },
    Deposit { tx: DepositTx },
    Exit { tx: ExitTx },
}

impl std::string::ToString for TransactionType {
    fn to_string(&self) -> String {
        match self {
            TransactionType::Transfer { tx: _ } => TRANSFER_TX.to_owned(),
            TransactionType::Deposit { tx: _ } => DEPOSIT_TX.to_owned(),
            TransactionType::Exit { tx: _ } => EXIT_TX.to_owned(),
        }
    }
}

/// Unpacked transaction data
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct TransferTx {
    pub from: u32,
    pub to: u32,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub nonce: u32,
    pub good_until_block: u32,
    pub signature: TxSignature,

    /// If present, it means that the signature has been verified against this key
    #[serde(skip)]
    pub cached_pub_key: Option<PublicKey>,
}

impl std::fmt::Debug for TransferTx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tx{{ from: {}, to: {}, nonce: {}, amount: {} }}",
            self.from, self.to, self.nonce, self.amount
        )?;
        write!(
            f,
            "tx{{ sigS: {}, sigR_x: {}, sigR_y: {} }}",
            self.signature.s, self.signature.r_x, self.signature.r_y
        )
    }
}

impl Ord for TransferTx {
    fn cmp(&self, other: &Self) -> Ordering {
        self.nonce.cmp(&other.nonce)
    }
}

impl PartialOrd for TransferTx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TransferTx {
    fn eq(&self, other: &Self) -> bool {
        self.nonce == other.nonce
    }
}

impl Eq for TransferTx {}

impl TransferTx {
    pub fn message_bits(&self) -> Vec<bool> {
        let mut r: Vec<bool> = vec![];
        let from_bits = get_bits_le_fixed_u128(self.from as u128, params::BALANCE_TREE_DEPTH);
        let to_bits = get_bits_le_fixed_u128(self.to as u128, params::BALANCE_TREE_DEPTH);
        let amount_bits = convert_to_float(
            self.amount.to_u128().unwrap(),
            params::AMOUNT_EXPONENT_BIT_WIDTH,
            params::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();
        let fee_bits = convert_to_float(
            self.fee.to_u128().unwrap(),
            params::FEE_EXPONENT_BIT_WIDTH,
            params::FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let nonce_bits = get_bits_le_fixed_u128(self.nonce as u128, params::NONCE_BIT_WIDTH);
        let good_until_block_bits = get_bits_le_fixed_u128(
            self.good_until_block as u128,
            params::BLOCK_NUMBER_BIT_WIDTH,
        );

        r.extend(from_bits.into_iter());
        r.extend(to_bits.into_iter());
        r.extend(amount_bits.into_iter());
        r.extend(fee_bits.into_iter());
        r.extend(nonce_bits.into_iter());
        r.extend(good_until_block_bits.into_iter());

        r
    }

    pub fn tx_data(&self) -> Option<Vec<u8>> {
        let message_bits = self.message_bits();

        if message_bits.len() % 8 != 0 {
            return None;
        }
        let as_bytes = pack_bits_into_bytes(message_bits);

        Some(as_bytes)
    }

    pub fn create_signed_tx(
        from: u32,
        to: u32,
        amount: BigDecimal,
        fee: BigDecimal,
        nonce: u32,
        good_until_block: u32,
        private_key: &PrivateKey,
    ) -> Self {
        let tx = TransferTx {
            from,
            to,
            amount: amount.clone(),
            fee: fee.clone(),
            nonce,
            good_until_block,
            signature: TxSignature::default(),
            cached_pub_key: None,
        };

        let message_bits = tx.message_bits();
        let as_bytes = pack_bits_into_bytes(message_bits);

        let rng = &mut rand::thread_rng();
        let p_g = FixedGenerators::SpendingKeyGenerator;

        let signature = TxSignature::from(private_key.sign_raw_message(
            &as_bytes,
            rng,
            p_g,
            &params::JUBJUB_PARAMS,
            as_bytes.len(),
        ));
        let cached_pub_key = Some(PublicKey::from_private(
            &private_key,
            p_g,
            &params::JUBJUB_PARAMS,
        ));

        TransferTx {
            from,
            to,
            amount,
            fee,
            nonce,
            good_until_block,
            signature,
            cached_pub_key,
        }
    }

    pub fn verify_sig(&self, public_key: &PublicKey) -> bool {
        let message_bits = self.message_bits();
        if message_bits.len() % 8 != 0 {
            println!("Invalid message length");
            return false;
        }
        let as_bytes = pack_bits_into_bytes(message_bits);
        //use rustc_hex::ToHex;
        //let hex: String = as_bytes.clone().to_hex();
        //println!("Transaction bytes = {}", hex);
        if let Ok(signature) = self.signature.to_jubjub_eddsa() {
            //println!("Successfuly converted to eddsa signature");
            let p_g = FixedGenerators::SpendingKeyGenerator;
            let valid = public_key.verify_for_raw_message(
                &as_bytes,
                &signature,
                p_g,
                &params::JUBJUB_PARAMS,
                30,
            );

            return valid;
        }
        //println!("Signature was not deserialized");

        false
    }

    pub fn validate(&self) -> Result<(), String> {
        use bigdecimal::Zero;
        if self.from == self.to {
            return Err(format!("tx.from may not equal tx.to: {}", self.from));
        }
        if self.amount == BigDecimal::zero() {
            return Err(format!("zero amount is not allowed"));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DepositTx {
    pub account: u32,
    pub amount: BigDecimal,
    pub pub_x: Fr,
    pub pub_y: Fr,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExitTx {
    pub account: u32,
    pub amount: BigDecimal,
}

#[derive(Clone, Default, Serialize, Deserialize)]
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

        Ok(Signature::<Engine> { r: r, s: s })
    }
}

impl TransactionSignature<Engine> {
    pub fn try_from(sig: crate::plasma::tx::TxSignature) -> Result<Self, String> {
        let r =
            edwards::Point::<Engine, Unknown>::from_xy(sig.r_x, sig.r_y, &params::JUBJUB_PARAMS)
                .expect("make R point");
        let s = sig.s;

        Ok(Self { r: r, s: s })
    }
}

impl Tx<Engine> {
    // TODO: introduce errors if necessary
    pub fn try_from(transaction: &crate::plasma::tx::TransferTx) -> Result<Self, String> {
        use bigdecimal::ToPrimitive;
        let encoded_amount_bits = convert_to_float(
            transaction.amount.to_u128().unwrap(), // TODO: use big decimal in convert_to_float() instead
            params::AMOUNT_EXPONENT_BIT_WIDTH,
            params::AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .map_err(|e| format!("wrong amount encoding: {}", e.to_string()))?;
        let encoded_amount: Fr = le_bit_vector_into_field_element(&encoded_amount_bits);

        let encoded_fee_bits = convert_to_float(
            transaction.fee.to_u128().unwrap(),
            params::FEE_EXPONENT_BIT_WIDTH,
            params::FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .map_err(|e| format!("wrong fee encoding: {}", e.to_string()))?;
        let encoded_fee: Fr = le_bit_vector_into_field_element(&encoded_fee_bits);

        let tx = Self {
            // TODO: these conversions are ugly and inefficient, replace with idiomatic std::convert::From trait
            from: Fr::from_str(&transaction.from.to_string()).unwrap(),
            to: Fr::from_str(&transaction.to.to_string()).unwrap(),
            amount: encoded_amount,
            fee: encoded_fee,
            nonce: Fr::from_str(&transaction.good_until_block.to_string()).unwrap(),
            good_until_block: Fr::from_str(&transaction.good_until_block.to_string()).unwrap(),

            signature: TransactionSignature::try_from(transaction.signature.clone())?,
        };

        Ok(tx)
    }
}

impl DepositRequest<Engine> {
    // TODO: introduce errors if necessary
    pub fn try_from(request: &crate::plasma::tx::DepositTx) -> Result<Self, String> {
        let req = Self {
            // TODO: these conversions are ugly and inefficient, replace with idiomatic std::convert::From trait
            into: Fr::from_str(&request.account.to_string()).unwrap(),
            amount: Fr::from_str(&request.amount.to_string()).unwrap(),
            pub_x: request.pub_x.clone(),
            pub_y: request.pub_y.clone(),
        };

        Ok(req)
    }
}

impl ExitRequest<Engine> {
    // TODO: introduce errors if necessary
    pub fn try_from(request: &crate::plasma::tx::ExitTx) -> Result<Self, String> {
        let req = Self {
            // TODO: these conversions are ugly and inefficient, replace with idiomatic std::convert::From trait
            from: Fr::from_str(&request.account.to_string()).unwrap(),
            amount: Fr::from_str(&request.amount.to_string()).unwrap(),
        };

        Ok(req)
    }
}
