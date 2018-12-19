use bigdecimal::{BigDecimal, ToPrimitive};
use crate::primitives::{get_bits_le_fixed_u128, pack_bits_into_bytes};
use pairing::bn256::{Bn256};
use sapling_crypto::jubjub::{JubjubEngine, JubjubParams, FixedGenerators, edwards};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};
use sapling_crypto::circuit::float_point::{convert_to_float};
use sapling_crypto::eddsa::{self, Signature};
use crate::models::circuit::sig::TransactionSignature;
use super::PublicKey;
use crate::models::params;
use ff::{Field, PrimeField, PrimeFieldRepr};
use super::{Fr, Engine};
use crate::circuit::utils::{encode_fr_into_fs};

/// Unpacked transaction data
#[derive(Clone, Serialize, Deserialize)]
pub struct TransferTx {
    pub from:               u32,
    pub to:                 u32,
    pub amount:             BigDecimal,
    pub fee:                BigDecimal,
    pub nonce:              u32,
    pub good_until_block:   u32,
    pub signature:          TxSignature,
}

impl TransferTx {
    pub fn message_bits(&self) -> Vec<bool> {
        let mut r: Vec<bool> = vec![];
        let from_bits = get_bits_le_fixed_u128(self.from as u128, params::BALANCE_TREE_DEPTH);
        let to_bits = get_bits_le_fixed_u128(self.to as u128, params::BALANCE_TREE_DEPTH);
        let amount_bits = convert_to_float(
                                    self.amount.to_u128().unwrap(), 
                                    params::AMOUNT_EXPONENT_BIT_WIDTH,
                                    params::AMOUNT_MANTISSA_BIT_WIDTH,
                                    10).unwrap();
        let fee_bits = convert_to_float(
                            self.fee.to_u128().unwrap(), 
                            params::FEE_EXPONENT_BIT_WIDTH,
                            params::FEE_MANTISSA_BIT_WIDTH,
                            10).unwrap();

        let nonce_bits = get_bits_le_fixed_u128(self.nonce as u128, params::NONCE_BIT_WIDTH);
        let good_until_block_bits = get_bits_le_fixed_u128(self.good_until_block as u128, params::BLOCK_NUMBER_BIT_WIDTH);

        r.extend(from_bits.into_iter());
        r.extend(to_bits.into_iter());
        r.extend(amount_bits.into_iter());
        r.extend(fee_bits.into_iter());
        r.extend(nonce_bits.into_iter());
        r.extend(good_until_block_bits.into_iter());

        r
    }

    pub fn verify_sig(
            &self, 
            public_key: PublicKey
        ) -> bool {
        let message_bits = self.message_bits();
        let as_bytes = pack_bits_into_bytes(message_bits);
        let signature = self.signature.to_jubjub_eddsa().expect("should parse signature");
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let valid = public_key.verify_for_raw_message(
            &as_bytes, 
            &signature, 
            p_g, 
            &params::JUBJUB_PARAMS, 
            16
        );

        valid
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DepositTx{
    pub account:            u32,
    pub amount:             BigDecimal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExitTx{
    pub account:            u32,
    pub amount:             BigDecimal,
}

// TxSignature uses only native Rust types
#[derive(Clone, Serialize, Deserialize)]
pub struct TxSignature{
    pub r_x: Fr,
    pub r_y: Fr,
    pub s: Fr,
}

impl TxSignature{
    pub fn try_from(
        signature: TransactionSignature<Engine>,
    ) -> Result<Self, String> {
        let (x, y) = signature.r.into_xy();

        Ok(Self{
            r_x: x,
            r_y: y,
            s: signature.s
        })
    }

    pub fn to_jubjub_eddsa(
        &self
    )
    -> Result<Signature<Engine>, String>
    {
        let r = edwards::Point::from_xy(self.r_x, self.r_y, &params::JUBJUB_PARAMS).expect("make point from X and Y");
        let s = encode_fr_into_fs(self.s);

        Ok(Signature::<Engine> {
            r: r,
            s: s
        })
    }
}
