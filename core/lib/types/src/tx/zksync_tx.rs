use crate::Nonce;

use crate::{
    tx::{ChangePubKey, Close, ForcedExit, Transfer, TxEthSignature, TxHash, Withdraw},
    CloseOp, ForcedExitOp, TokenLike, TransferOp, TxFeeTypes, WithdrawOp,
};
use num::BigUint;
use parity_crypto::digest::sha256;

use crate::operations::ChangePubKeyOp;
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EthSignData {
    pub signature: TxEthSignature,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedZkSyncTx {
    pub tx: ZkSyncTx,
    pub eth_sign_data: Option<EthSignData>,
}

/// A set of L2 transaction supported by the zkSync network.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ZkSyncTx {
    Transfer(Box<Transfer>),
    Withdraw(Box<Withdraw>),
    #[doc(hidden)]
    Close(Box<Close>),
    ChangePubKey(Box<ChangePubKey>),
    ForcedExit(Box<ForcedExit>),
}

impl From<Transfer> for ZkSyncTx {
    fn from(transfer: Transfer) -> Self {
        Self::Transfer(Box::new(transfer))
    }
}

impl From<Withdraw> for ZkSyncTx {
    fn from(withdraw: Withdraw) -> Self {
        Self::Withdraw(Box::new(withdraw))
    }
}

impl From<Close> for ZkSyncTx {
    fn from(close: Close) -> Self {
        Self::Close(Box::new(close))
    }
}

impl From<ChangePubKey> for ZkSyncTx {
    fn from(change_pub_key: ChangePubKey) -> Self {
        Self::ChangePubKey(Box::new(change_pub_key))
    }
}

impl From<ForcedExit> for ZkSyncTx {
    fn from(tx: ForcedExit) -> Self {
        Self::ForcedExit(Box::new(tx))
    }
}

impl From<ZkSyncTx> for SignedZkSyncTx {
    fn from(tx: ZkSyncTx) -> Self {
        Self {
            tx,
            eth_sign_data: None,
        }
    }
}

impl std::ops::Deref for SignedZkSyncTx {
    type Target = ZkSyncTx;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl ZkSyncTx {
    /// Returns the hash of the transaction.
    pub fn hash(&self) -> TxHash {
        let bytes = match self {
            ZkSyncTx::Transfer(tx) => tx.get_bytes(),
            ZkSyncTx::Withdraw(tx) => tx.get_bytes(),
            ZkSyncTx::Close(tx) => tx.get_bytes(),
            ZkSyncTx::ChangePubKey(tx) => tx.get_bytes(),
            ZkSyncTx::ForcedExit(tx) => tx.get_bytes(),
        };

        let hash = sha256(&bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hash);
        TxHash { data: out }
    }

    /// Returns the account affected by the transaction.
    pub fn account(&self) -> Address {
        match self {
            ZkSyncTx::Transfer(tx) => tx.from,
            ZkSyncTx::Withdraw(tx) => tx.from,
            ZkSyncTx::Close(tx) => tx.account,
            ZkSyncTx::ChangePubKey(tx) => tx.account,
            ZkSyncTx::ForcedExit(tx) => tx.target,
        }
    }

    /// Returns the account nonce associated with transaction.
    pub fn nonce(&self) -> Nonce {
        match self {
            ZkSyncTx::Transfer(tx) => tx.nonce,
            ZkSyncTx::Withdraw(tx) => tx.nonce,
            ZkSyncTx::Close(tx) => tx.nonce,
            ZkSyncTx::ChangePubKey(tx) => tx.nonce,
            ZkSyncTx::ForcedExit(tx) => tx.nonce,
        }
    }

    /// Checks whether transaction is well-formed and can be executed.
    ///
    /// Note that this method doesn't check whether transaction will succeed, so transaction
    /// can fail even if this method returned `true` (i.e., if account didn't have enough balance).
    pub fn check_correctness(&mut self) -> bool {
        match self {
            ZkSyncTx::Transfer(tx) => tx.check_correctness(),
            ZkSyncTx::Withdraw(tx) => tx.check_correctness(),
            ZkSyncTx::Close(tx) => tx.check_correctness(),
            ZkSyncTx::ChangePubKey(tx) => tx.check_correctness(),
            ZkSyncTx::ForcedExit(tx) => tx.check_correctness(),
        }
    }

    /// Encodes the transaction data as the byte sequence according to the zkSync protocol.
    pub fn get_bytes(&self) -> Vec<u8> {
        match self {
            ZkSyncTx::Transfer(tx) => tx.get_bytes(),
            ZkSyncTx::Withdraw(tx) => tx.get_bytes(),
            ZkSyncTx::Close(tx) => tx.get_bytes(),
            ZkSyncTx::ChangePubKey(tx) => tx.get_bytes(),
            ZkSyncTx::ForcedExit(tx) => tx.get_bytes(),
        }
    }

    /// Returns the minimum amount of block chunks required for this operation.
    /// Maximum amount of chunks in block is a part of  the server and provers configuration,
    /// and this value determines the block capacity.
    pub fn min_chunks(&self) -> usize {
        match self {
            ZkSyncTx::Transfer(_) => TransferOp::CHUNKS,
            ZkSyncTx::Withdraw(_) => WithdrawOp::CHUNKS,
            ZkSyncTx::Close(_) => CloseOp::CHUNKS,
            ZkSyncTx::ChangePubKey(_) => ChangePubKeyOp::CHUNKS,
            ZkSyncTx::ForcedExit(_) => ForcedExitOp::CHUNKS,
        }
    }

    /// Returns `true` if transaction is `ZkSyncTx::Withdraw`.
    pub fn is_withdraw(&self) -> bool {
        matches!(self, ZkSyncTx::Withdraw(_) | ZkSyncTx::ForcedExit(_))
    }

    /// Returns `true` if transaction is `ZkSyncTx::Withdraw`.
    #[doc(hidden)]
    pub fn is_close(&self) -> bool {
        matches!(self, ZkSyncTx::Close(_))
    }

    /// Returns the data required to calculate fee for the transaction.
    ///
    /// Response includes the following items:
    ///
    /// - Fee type.
    /// - Token to pay fees in.
    /// - Address of account affected by the transaction.
    /// - Fee provided in the transaction.
    ///
    /// Returns `None` if transaction doesn't require fee.
    pub fn get_fee_info(&self) -> Option<(TxFeeTypes, TokenLike, Address, BigUint)> {
        match self {
            ZkSyncTx::Withdraw(withdraw) => {
                let fee_type = if withdraw.fast {
                    TxFeeTypes::FastWithdraw
                } else {
                    TxFeeTypes::Withdraw
                };

                Some((
                    fee_type,
                    TokenLike::Id(withdraw.token),
                    withdraw.to,
                    withdraw.fee.clone(),
                ))
            }
            ZkSyncTx::ForcedExit(forced_exit) => Some((
                TxFeeTypes::Withdraw,
                TokenLike::Id(forced_exit.token),
                forced_exit.target,
                forced_exit.fee.clone(),
            )),
            ZkSyncTx::Transfer(transfer) => Some((
                TxFeeTypes::Transfer,
                TokenLike::Id(transfer.token),
                transfer.to,
                transfer.fee.clone(),
            )),
            ZkSyncTx::ChangePubKey(change_pubkey) => Some((
                TxFeeTypes::ChangePubKey {
                    onchain_pubkey_auth: change_pubkey.eth_signature.is_none(),
                },
                TokenLike::Id(change_pubkey.fee_token),
                change_pubkey.account,
                change_pubkey.fee.clone(),
            )),
            _ => None,
        }
    }
}
