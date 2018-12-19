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
        let signature = self.signature.to_jubjub_eddsa(&*params::JUBJUB_PARAMS).expect("should parse signature");
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
    pub r_compressed:    [u8; 32], // top bit is a sign
    pub s:               [u8; 32],
}

impl TxSignature{
    pub fn try_from<E: JubjubEngine>(
        signature: TransactionSignature<E>,
    ) -> Result<Self, String> {
        let mut tmp = TxSignature{
            r_compressed: [0u8; 32],
            s: [0u8; 32]
        };
        let (y, sign) = signature.r.compress_into_y();
        y.into_repr().write_be(& mut tmp.r_compressed[..]).expect("write y");
        if sign {
            tmp.r_compressed[0] |= 0x80
        }

        signature.s.into_repr().write_be(& mut tmp.s[..]).expect("write s");

        Ok(tmp)
    }

    pub fn to_jubjub_eddsa<E: JubjubEngine>(
        &self, 
        params: &E::Params
    )
    -> Result<Signature<E>, String>
    {
        // TxSignature has S and R in compressed form serialized as BE
        let x_sign = self.r_compressed[0] & 0x80 > 0;
        let mut tmp = self.r_compressed.clone();
        tmp[0] &= 0x7f; // strip the top bit

        // read from byte array
        let mut y_repr = E::Fr::zero().into_repr();
        y_repr.read_be(&tmp[..]).expect("read R_y as field element");

        let mut s_repr = E::Fs::zero().into_repr();
        s_repr.read_be(&self.s[..]).expect("read S as field element");

        let y = E::Fr::from_repr(y_repr).expect("make y from representation");

        // here we convert it to field elements for all further uses
        let r = edwards::Point::get_for_y(y, x_sign, params);
        if r.is_none() {
            return Err("Invalid R point".to_string());
        }

        let s = E::Fs::from_repr(s_repr).expect("make s from representation");

        Ok(Signature {
            r: r.unwrap(),
            s: s
        })
    }
}
