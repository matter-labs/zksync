use num::BigUint;
use zksync_eth_signer::EthereumSigner;
use zksync_types::{
    helpers::{closest_packable_fee_amount, is_fee_amount_packable},
    tokens::TxFeeTypes,
    Nonce, Token, TokenLike, ZkSyncTx,
};

use crate::{error::ClientError, operations::SyncTransactionHandle, wallet::Wallet};

#[derive(Debug)]
pub struct ChangePubKeyBuilder<'a, S: EthereumSigner> {
    wallet: &'a Wallet<S>,
    onchain_auth: bool,
    fee_token: Option<Token>,
    fee: Option<BigUint>,
    nonce: Option<Nonce>,
}

impl<'a, S: EthereumSigner + Clone> ChangePubKeyBuilder<'a, S> {
    /// Initializes a change public key transaction building process.
    pub fn new(wallet: &'a Wallet<S>) -> Self {
        Self {
            wallet,
            onchain_auth: false,
            fee_token: None,
            fee: None,
            nonce: None,
        }
    }

    /// Sends the transaction, returning the handle for its awaiting.
    pub async fn tx(self) -> Result<ZkSyncTx, ClientError> {
        let fee_token = self
            .fee_token
            .ok_or_else(|| ClientError::MissingRequiredField("fee_token".into()))?;

        let fee = match self.fee {
            Some(fee) => fee,
            None => {
                let fee = self
                    .wallet
                    .provider
                    .get_tx_fee(
                        TxFeeTypes::ChangePubKey {
                            onchain_pubkey_auth: self.onchain_auth,
                        },
                        self.wallet.address(),
                        fee_token.id,
                    )
                    .await?;
                fee.total_fee
            }
        };

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

        Ok(ZkSyncTx::from(
            self.wallet
                .signer
                .sign_change_pubkey_tx(nonce, self.onchain_auth, fee_token, fee)
                .await
                .map_err(ClientError::SigningError)?,
        ))
    }

    /// Sends the transaction, returning the handle for its awaiting.
    pub async fn send(self) -> Result<SyncTransactionHandle, ClientError> {
        let provider = self.wallet.provider.clone();

        let tx = self.tx().await?;
        let tx_hash = provider.send_tx(tx, None).await?;

        Ok(SyncTransactionHandle::new(tx_hash, provider))
    }

    /// Sets the transaction fee token. Returns an error if token is not supported by zkSync.
    pub fn fee_token(mut self, token: impl Into<TokenLike>) -> Result<Self, ClientError> {
        let token_like = token.into();
        let token = self
            .wallet
            .tokens
            .resolve(token_like)
            .ok_or(ClientError::UnknownToken)?;

        self.fee_token = Some(token);

        Ok(self)
    }

    /// Set the fee amount. If the amount provided is not packable,
    /// rounds it to the closest packable fee amount.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn fee(mut self, fee: impl Into<BigUint>) -> Self {
        let fee = closest_packable_fee_amount(&fee.into());
        self.fee = Some(fee);

        self
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

    /// Sets the transaction nonce.
    pub fn nonce(mut self, nonce: Nonce) -> Self {
        self.nonce = Some(nonce);
        self
    }
}
