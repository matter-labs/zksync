use num::BigUint;
use zksync_eth_signer::EthereumSigner;
use zksync_types::{
    helpers::{
        closest_packable_fee_amount, closest_packable_token_amount, is_fee_amount_packable,
        is_token_amount_packable,
    },
    tx::PackedEthSignature,
    Address, Nonce, Token, TokenLike, TxFeeTypes, ZkSyncTx,
};

use crate::{error::ClientError, operations::SyncTransactionHandle, wallet::Wallet};

#[derive(Debug)]
pub struct TransferBuilder<'a, S: EthereumSigner> {
    wallet: &'a Wallet<S>,
    token: Option<Token>,
    amount: Option<BigUint>,
    fee: Option<BigUint>,
    to: Option<Address>,
    nonce: Option<Nonce>,
}

impl<'a, S: EthereumSigner + Clone> TransferBuilder<'a, S> {
    /// Initializes a transfer transaction building process.
    pub fn new(wallet: &'a Wallet<S>) -> Self {
        Self {
            wallet,
            token: None,
            amount: None,
            fee: None,
            to: None,
            nonce: None,
        }
    }

    /// Directly returns the signed transfer transaction for the subsequent usage.
    pub async fn tx(self) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let token = self
            .token
            .ok_or_else(|| ClientError::MissingRequiredField("token".into()))?;
        let amount = self
            .amount
            .ok_or_else(|| ClientError::MissingRequiredField("amount".into()))?;
        let to = self
            .to
            .ok_or_else(|| ClientError::MissingRequiredField("to".into()))?;

        let nonce = match self.nonce {
            Some(nonce) => nonce,
            None => {
                let account_info = self
                    .wallet
                    .provider
                    .account_info(self.wallet.address())
                    .await?;
                account_info.committed.nonce
            }
        };

        let fee = match self.fee {
            Some(fee) => fee,
            None => {
                let fee = self
                    .wallet
                    .provider
                    .get_tx_fee(TxFeeTypes::Transfer, to, token.id)
                    .await?;
                fee.total_fee
            }
        };

        self.wallet
            .signer
            .sign_transfer(token, amount, fee, to, nonce)
            .await
            .map(|(tx, signature)| (ZkSyncTx::Transfer(Box::new(tx)), signature))
            .map_err(ClientError::SigningError)
    }

    /// Sends the transaction, returning the handle for its awaiting.
    pub async fn send(self) -> Result<SyncTransactionHandle, ClientError> {
        let provider = self.wallet.provider.clone();

        let (tx, eth_signature) = self.tx().await?;
        let tx_hash = provider.send_tx(tx, eth_signature).await?;

        Ok(SyncTransactionHandle::new(tx_hash, provider))
    }

    /// Sets the transaction token. Returns an error if token is not supported by zkSync.
    pub fn token(mut self, token: impl Into<TokenLike>) -> Result<Self, ClientError> {
        let token_like = token.into();
        let token = self
            .wallet
            .tokens
            .resolve(token_like)
            .ok_or(ClientError::UnknownToken)?;

        self.token = Some(token);

        Ok(self)
    }

    /// Set the transfer amount. If the provided amount is not packable,
    /// rounds it to the closest packable amount.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn amount(mut self, amount: impl Into<BigUint>) -> Self {
        let amount = closest_packable_token_amount(&amount.into());
        self.amount = Some(amount);

        self
    }

    /// Set the fee amount. If the provided fee is not packable,
    /// rounds it to the closest packable fee amount.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn fee(mut self, fee: impl Into<BigUint>) -> Self {
        let fee = closest_packable_fee_amount(&fee.into());
        self.fee = Some(fee);

        self
    }

    /// Set the transfer amount. If the provided amount is not packable,
    /// returns an error.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn amount_exact(mut self, amount: impl Into<BigUint>) -> Result<Self, ClientError> {
        let amount = amount.into();
        if !is_token_amount_packable(&amount) {
            return Err(ClientError::NotPackableValue);
        }
        self.amount = Some(amount);

        Ok(self)
    }

    /// Set the fee amount. If the provided fee is not packable,
    /// returns an error.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn fee_exact(mut self, fee: impl Into<BigUint>) -> Result<Self, ClientError> {
        let fee = fee.into();
        if !is_fee_amount_packable(&fee) {
            return Err(ClientError::NotPackableValue);
        }
        self.fee = Some(fee);

        Ok(self)
    }

    /// Sets the transaction recipient.
    pub fn to(mut self, to: Address) -> Self {
        self.to = Some(to);
        self
    }

    /// Same as `TransferBuilder::to`, but accepts a string address value.
    ///
    /// Provided string value must be a correct address in a hexadecimal form,
    /// otherwise an error will be returned.
    pub fn str_to(mut self, to: impl AsRef<str>) -> Result<Self, ClientError> {
        let to: Address = to
            .as_ref()
            .parse()
            .map_err(|_| ClientError::IncorrectAddress)?;

        self.to = Some(to);
        Ok(self)
    }

    /// Sets the transaction nonce.
    pub fn nonce(mut self, nonce: Nonce) -> Self {
        self.nonce = Some(nonce);
        self
    }
}
