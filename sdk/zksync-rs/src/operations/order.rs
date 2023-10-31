use num::BigUint;
use zksync_eth_signer::EthereumSigner;
use zksync_types::{
    helpers::{closest_packable_token_amount, is_token_amount_packable},
    tx::TimeRange,
    Address, Nonce, Order, Token, TokenLike,
};

use crate::{error::ClientError, provider::Provider, wallet::Wallet};

#[derive(Debug)]
pub struct OrderBuilder<'a, S: EthereumSigner, P: Provider> {
    wallet: &'a Wallet<S, P>,
    recipient: Option<Address>,
    nonce: Option<Nonce>,
    token_sell: Option<Token>,
    token_buy: Option<Token>,
    prices: Option<(BigUint, BigUint)>,
    amount: Option<BigUint>,
    valid_from: Option<u64>,
    valid_until: Option<u64>,
}

impl<'a, S, P> OrderBuilder<'a, S, P>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    /// Initializes a norder building process.
    pub fn new(wallet: &'a Wallet<S, P>) -> Self {
        Self {
            wallet,
            recipient: None,
            nonce: None,
            token_sell: None,
            token_buy: None,
            prices: None,
            amount: None,
            valid_from: None,
            valid_until: None,
        }
    }

    /// Different from other operations, OrderBuilder only builds the order,
    /// which might then be used to assemble a Swap transaction.
    pub async fn order(self) -> Result<Order, ClientError> {
        let recipient = self
            .recipient
            .ok_or_else(|| ClientError::MissingRequiredField("recipient".into()))?;
        let token_sell = self
            .token_sell
            .ok_or_else(|| ClientError::MissingRequiredField("token_sell".into()))?;
        let token_buy = self
            .token_buy
            .ok_or_else(|| ClientError::MissingRequiredField("token_buy".into()))?;
        let prices = self
            .prices
            .ok_or_else(|| ClientError::MissingRequiredField("prices".into()))?;
        let amount = self
            .amount
            .ok_or_else(|| ClientError::MissingRequiredField("amount".into()))?;
        let valid_from = self.valid_from.unwrap_or(0);
        let valid_until = self.valid_until.unwrap_or(u64::MAX);

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

        self.wallet
            .signer
            .sign_order(
                recipient,
                nonce,
                token_sell,
                token_buy,
                prices,
                amount,
                TimeRange::new(valid_from, valid_until),
            )
            .await
            .map_err(ClientError::SigningError)
    }

    /// Sets the order recipient.
    pub fn recipient(mut self, recipient: Address) -> Self {
        self.recipient = Some(recipient);
        self
    }

    /// Same as `OrderBuilder::recipient`, but accepts a string address value.
    ///
    /// Provided string value must be a correct address in a hexadecimal form,
    /// otherwise an error will be returned.
    pub fn str_recipient(mut self, recipient: impl AsRef<str>) -> Result<Self, ClientError> {
        let recipient: Address = recipient
            .as_ref()
            .parse()
            .map_err(|_| ClientError::IncorrectAddress)?;

        self.recipient = Some(recipient);
        Ok(self)
    }

    /// Sets the order nonce.
    pub fn nonce(mut self, nonce: Nonce) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Sets the token to sell. Returns an error if token is not supported by zkSync.
    pub fn token_sell(mut self, token: impl Into<TokenLike>) -> Result<Self, ClientError> {
        let token_like = token.into();
        let token = self
            .wallet
            .tokens
            .resolve(token_like)
            .ok_or(ClientError::UnknownToken)?;

        self.token_sell = Some(token);

        Ok(self)
    }

    /// Sets the token to buy. Returns an error if token is not supported by zkSync.
    pub fn token_buy(mut self, token: impl Into<TokenLike>) -> Result<Self, ClientError> {
        let token_like = token.into();
        let token = self
            .wallet
            .tokens
            .resolve(token_like)
            .ok_or(ClientError::UnknownToken)?;

        self.token_buy = Some(token);

        Ok(self)
    }

    /// Set the order prices. If the provided prices are not packable,
    /// rounds them to the closest packable prices.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn prices(mut self, prices: (impl Into<BigUint>, impl Into<BigUint>)) -> Self {
        let price0 = closest_packable_token_amount(&prices.0.into());
        let price1 = closest_packable_token_amount(&prices.1.into());
        self.prices = Some((price0, price1));

        self
    }

    /// Set the order prices. If either of the provided price is not packable,
    /// returns an error.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn prices_exact(
        mut self,
        prices: (impl Into<BigUint>, impl Into<BigUint>),
    ) -> Result<Self, ClientError> {
        let price0 = prices.0.into();
        if !is_token_amount_packable(&price0) {
            return Err(ClientError::NotPackableValue);
        }
        let price1 = prices.1.into();
        if !is_token_amount_packable(&price1) {
            return Err(ClientError::NotPackableValue);
        }
        self.prices = Some((price0, price1));

        Ok(self)
    }

    /// Set the order amount. If the provided amount is not packable,
    /// rounds it to the closest packable amount.
    ///
    /// For more details, see [utils](../utils/index.html) functions.
    pub fn amount(mut self, amount: impl Into<BigUint>) -> Self {
        let amount = closest_packable_token_amount(&amount.into());
        self.amount = Some(amount);

        self
    }

    /// Set the order amount. If the provided amount is not packable,
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

    /// Sets the unix format timestamp of the first moment when order execution is valid.
    pub fn valid_from(mut self, valid_from: u64) -> Self {
        self.valid_from = Some(valid_from);
        self
    }

    /// Sets the unix format timestamp of the last moment when order execution is valid.
    pub fn valid_until(mut self, valid_until: u64) -> Self {
        self.valid_until = Some(valid_until);
        self
    }
}
