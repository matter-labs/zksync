use num::BigUint;
use parity_crypto::digest::sha256;
use serde::{Deserialize, Serialize};

use zksync_basic_types::{AccountId, Address};

use crate::{
    operations::ChangePubKeyOp,
    tx::{ChangePubKey, Close, ForcedExit, Transfer, TxEthSignature, TxHash, Withdraw},
    utils::deserialize_eth_message,
    CloseOp, ForcedExitOp, Nonce, Token, TokenId, TokenLike, TransferOp, TxFeeTypes, WithdrawOp,
};
use zksync_crypto::params::ETH_TOKEN_ID;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EthSignData {
    pub signature: TxEthSignature,
    #[serde(deserialize_with = "deserialize_eth_message")]
    pub message: Vec<u8>,
}

/// Represents transaction with the corresponding Ethereum signature and the message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedZkSyncTx {
    /// Underlying zkSync transaction.
    pub tx: ZkSyncTx,
    /// `eth_sign_data` is a tuple of the Ethereum signature and the message
    /// which user should have signed with their private key.
    /// Can be `None` if the Ethereum signature is not required.
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

    pub fn account_id(&self) -> anyhow::Result<AccountId> {
        match self {
            ZkSyncTx::Transfer(tx) => Ok(tx.account_id),
            ZkSyncTx::Withdraw(tx) => Ok(tx.account_id),
            ZkSyncTx::ChangePubKey(tx) => Ok(tx.account_id),
            ZkSyncTx::ForcedExit(tx) => Ok(tx.initiator_account_id),
            ZkSyncTx::Close(_) => Err(anyhow::anyhow!("Close operations are disabled")),
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

    /// Returns the token used to pay the transaction fee with.
    ///
    /// For `Close` we return 0 and expect the server to decline
    /// the transaction before the call to this method.
    pub fn token_id(&self) -> TokenId {
        match self {
            ZkSyncTx::Transfer(tx) => tx.token,
            ZkSyncTx::Withdraw(tx) => tx.token,
            ZkSyncTx::Close(_) => ETH_TOKEN_ID,
            ZkSyncTx::ChangePubKey(tx) => tx.fee_token,
            ZkSyncTx::ForcedExit(tx) => tx.token,
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

    /// Returns a message that user has to sign to send the transaction.
    /// If the transaction doesn't need a message signature, returns `None`.
    /// `ChangePubKey` message is handled separately since its Ethereum signature
    /// is passed to the contract.
    pub fn get_ethereum_sign_message(&self, token: Token) -> Option<String> {
        match self {
            ZkSyncTx::Transfer(tx) => {
                Some(tx.get_ethereum_sign_message(&token.symbol, token.decimals))
            }
            ZkSyncTx::Withdraw(tx) => {
                Some(tx.get_ethereum_sign_message(&token.symbol, token.decimals))
            }
            ZkSyncTx::ForcedExit(tx) => {
                Some(tx.get_ethereum_sign_message(&token.symbol, token.decimals))
            }
            _ => None,
        }
    }

    /// Returns a message that user has to sign to send the transaction in the old format.
    /// If the transaction doesn't need a message signature, returns `None`.
    /// Needed for backwards compatibility.
    pub fn get_old_ethereum_sign_message(&self, token: Token) -> Option<String> {
        match self {
            ZkSyncTx::Transfer(tx) => {
                Some(tx.get_old_ethereum_sign_message(&token.symbol, token.decimals))
            }
            ZkSyncTx::Withdraw(tx) => {
                Some(tx.get_old_ethereum_sign_message(&token.symbol, token.decimals))
            }
            _ => None,
        }
    }

    /// Returns the corresponding part of the batch message user has to sign in order
    /// to send it. In this case we handle `ChangePubKey` on the server side and
    /// expect a line in the message for it.
    pub fn get_ethereum_sign_message_part(&self, token: Token) -> Option<String> {
        match self {
            ZkSyncTx::Transfer(tx) => {
                Some(tx.get_ethereum_sign_message_part(&token.symbol, token.decimals))
            }
            ZkSyncTx::Withdraw(tx) => {
                Some(tx.get_ethereum_sign_message_part(&token.symbol, token.decimals))
            }
            ZkSyncTx::ChangePubKey(tx) => {
                Some(tx.get_ethereum_sign_message_part(&token.symbol, token.decimals))
            }
            ZkSyncTx::ForcedExit(tx) => {
                Some(tx.get_ethereum_sign_message_part(&token.symbol, token.decimals))
            }
            _ => None,
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
                change_pubkey.get_fee_type(),
                TokenLike::Id(change_pubkey.fee_token),
                change_pubkey.account,
                change_pubkey.fee.clone(),
            )),
            _ => None,
        }
    }

    /// Returns the unix format timestamp of the first moment when transaction execution is valid.
    pub fn valid_from(&self) -> u64 {
        match self {
            ZkSyncTx::Transfer(tx) => tx.time_range.unwrap_or_default().valid_from,
            ZkSyncTx::Withdraw(tx) => tx.time_range.unwrap_or_default().valid_from,
            ZkSyncTx::ChangePubKey(tx) => tx.time_range.unwrap_or_default().valid_from,
            ZkSyncTx::ForcedExit(tx) => tx.time_range.valid_from,
            ZkSyncTx::Close(tx) => tx.time_range.valid_from,
        }
    }
}
