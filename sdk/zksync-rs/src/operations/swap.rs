use num::{BigUint, Zero};
use zksync_eth_signer::EthereumSigner;
use zksync_types::{
    helpers::{
        closest_packable_fee_amount, closest_packable_token_amount, is_fee_amount_packable,
        is_token_amount_packable,
    },
    tx::PackedEthSignature,
    Nonce, Order, Token, TokenLike, TxFeeTypes, ZkSyncTx,
};

use crate::{
    error::ClientError, operations::SyncTransactionHandle, provider::Provider, wallet::Wallet,
};

#[derive(Debug)]
pub struct SwapBuilder<'a, S: EthereumSigner, P: Provider> {
    wallet: &'a Wallet<S, P>,
    nonce: Option<Nonce>,
    orders: Option<(Order, Order)>,
    amounts: Option<(BigUint, BigUint)>,
    fee: Option<BigUint>,
    fee_token: Option<Token>,
}

impl<'a, S, P> SwapBuilder<'a, S, P>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    /// Initializes a swap transaction building process.
    pub fn new(wallet: &'a Wallet<S, P>) -> Self {
        Self {
            wallet,
            nonce: None,
            orders: None,
            amounts: None,
            fee: None,
            fee_token: None,
        }
    }

    /// Directly returns the signed swap transaction for the subsequent usage.
    pub async fn tx(self) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let orders = self
            .orders
            .ok_or_else(|| ClientError::MissingRequiredField("orders".into()))?;
        let fee_token = self
            .fee_token
            .ok_or_else(|| ClientError::MissingRequiredField("fee_token".into()))?;

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
                    .get_tx_fee(TxFeeTypes::Transfer, self.wallet.address(), fee_token.id)
                    .await?;
                fee.total_fee
            }
        };

        let amounts = match self.amounts {
            Some(amounts) => amounts,
            None => {
                let amount0 = orders.0.amount.clone();
                let amount1 = orders.1.amount.clone();
                if amount0.is_zero() && amount1.is_zero() {
                    return Err(ClientError::MissingRequiredField("amounts".into()));
                }
                (amount0, amount1)
            }
        };

        self.wallet
            .signer
            .sign_swap(nonce, orders, amounts, fee, fee_token)
            .await
            .map(|(tx, signature)| (ZkSyncTx::Swap(Box::new(tx)), signature))
            .map_err(ClientError::SigningError)
    }

    /// Sends the transaction, returning the handle for its awaiting.
    pub async fn send(self) -> Result<SyncTransactionHandle<P>, ClientError> {
        let provider = self.wallet.provider.clone();

        let (tx, eth_signature) = self.tx().await?;
        let tx_hash = provider.send_tx(tx, eth_signature).await?;

        Ok(SyncTransactionHandle::new(tx_hash, provider))
    }

    /// Sets the transaction nonce.
    pub fn nonce(mut self, nonce: Nonce) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Sets orders to swap
    pub fn orders(mut self, orders: (Order, Order)) -> Self {
        self.orders = Some(orders);
        self
    }

    /// Set the order fill amounts. If the provided amounts are not packable,
    /// rounds them to the closest packable amount.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn amounts(mut self, amounts: (impl Into<BigUint>, impl Into<BigUint>)) -> Self {
        let amount0 = closest_packable_token_amount(&amounts.0.into());
        let amount1 = closest_packable_token_amount(&amounts.1.into());
        self.amounts = Some((amount0, amount1));

        self
    }

    /// Set the order fill amounts. If either of the provided amount is not packable,
    /// returns an error.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn amounts_exact(
        mut self,
        amounts: (impl Into<BigUint>, impl Into<BigUint>),
    ) -> Result<Self, ClientError> {
        let amount0 = amounts.0.into();
        if !is_token_amount_packable(&amount0) {
            return Err(ClientError::NotPackableValue);
        }
        let amount1 = amounts.1.into();
        if !is_token_amount_packable(&amount1) {
            return Err(ClientError::NotPackableValue);
        }
        self.amounts = Some((amount0, amount1));

        Ok(self)
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

    /// Sets the fee token. Returns an error if token is not supported by zkSync.
    pub fn fee_token(mut self, fee_token: impl Into<TokenLike>) -> Result<Self, ClientError> {
        let token_like = fee_token.into();
        let token = self
            .wallet
            .tokens
            .resolve(token_like)
            .ok_or(ClientError::UnknownToken)?;

        self.fee_token = Some(token);

        Ok(self)
    }
}
