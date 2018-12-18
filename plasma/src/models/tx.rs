use bigdecimal::BigDecimal;
use super::TxSignature;

/// Unpacked transaction data
#[derive(Clone, Serialize, Deserialize)]
pub struct TransferTx{
    pub from:               u32,
    pub to:                 u32,
    pub amount:             u128,
    pub fee:                u128,
    pub nonce:              u32,
    pub good_until_block:   u32,
    pub signature:          TxSignature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DepositTx{
    pub account:            u32,
    pub amount:             u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExitTx{
    pub account:            u32,
    pub amount:             u128,
}
