use num::BigUint;
use zksync_crypto::params::MIN_NFT_TOKEN_ID;
use zksync_eth_signer::{error::SignerError, EthereumSigner};
use zksync_types::{
    helpers::{closest_packable_fee_amount, is_fee_amount_packable},
    tx::{PackedEthSignature, TxEthSignature, TxHash},
    Address, Nonce, Token, TokenId, TokenLike, TxFeeTypes, ZkSyncTx,
};

use crate::{error::ClientError, provider::Provider, wallet::Wallet};
use zksync_types::tx::TimeRange;

#[derive(Debug)]
pub struct TransferNFTBuilder<'a, S: EthereumSigner, P: Provider> {
    wallet: &'a Wallet<S, P>,
    token: Option<TokenId>,
    fee_token: Option<Token>,
    fee: Option<BigUint>,
    to: Option<Address>,
    nonce: Option<Nonce>,
    valid_from: Option<u64>,
    valid_until: Option<u64>,
}

impl<'a, S, P> TransferNFTBuilder<'a, S, P>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    /// Initializes a transfer nft transaction batch building process.
    pub fn new(wallet: &'a Wallet<S, P>) -> Self {
        Self {
            wallet,
            token: None,
            fee_token: None,
            fee: None,
            to: None,
            nonce: None,
            valid_from: None,
            valid_until: None,
        }
    }

    /// Directly returns the couple of transfer transactions for the subsequent usage.
    pub async fn tx(self) -> Result<(ZkSyncTx, ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let token = self
            .token
            .ok_or_else(|| ClientError::MissingRequiredField("token".into()))?;
        let fee_token = self
            .fee_token
            .ok_or_else(|| ClientError::MissingRequiredField("fee_token".into()))?;
        let to = self
            .to
            .ok_or_else(|| ClientError::MissingRequiredField("to".into()))?;
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

        let fee = match self.fee {
            Some(fee) => fee,
            None => {
                let fee = self
                    .wallet
                    .provider
                    .get_tx_fee(TxFeeTypes::Transfer, to, fee_token.id)
                    .await?;
                fee.total_fee
            }
        };

        let token = self
            .wallet
            .tokens
            .resolve(TokenLike::Id(token))
            .ok_or(ClientError::UnknownToken)?;
        let tx_nft = self
            .wallet
            .signer
            .sign_transfer(
                token.clone(),
                BigUint::from(1u16),
                BigUint::from(0u16),
                to,
                nonce,
                TimeRange::new(valid_from, valid_until),
            )
            .await
            .map(|(tx, _)| ZkSyncTx::Transfer(Box::new(tx)))
            .map_err(ClientError::SigningError)?;
        let tx_fee = self
            .wallet
            .signer
            .sign_transfer(
                fee_token.clone(),
                BigUint::from(0u16),
                fee,
                to,
                nonce + 1,
                TimeRange::new(valid_from, valid_until),
            )
            .await
            .map(|(tx, _)| ZkSyncTx::Transfer(Box::new(tx)))
            .map_err(ClientError::SigningError)?;
        let message_part_1 = tx_nft.get_ethereum_sign_message_part(token).unwrap();
        let message_part_2 = tx_fee.get_ethereum_sign_message_part(fee_token).unwrap();
        let message = format!("{}\n{}\nNonce: {}", message_part_1, message_part_2, nonce);

        let eth_signature = match &self.wallet.signer.eth_signer {
            Some(signer) => {
                let signature = signer
                    .sign_message(&message.as_bytes())
                    .await
                    .map_err(ClientError::SigningError)?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(ClientError::SigningError(SignerError::MissingEthSigner));
                }
            }
            _ => None,
        };

        Ok((tx_nft, tx_fee, eth_signature))
    }

    /// Sends the transaction batch, returning the hashes of its transactions.
    pub async fn send(self) -> Result<Vec<TxHash>, ClientError> {
        let provider = self.wallet.provider.clone();

        let (tx_nft, tx_fee, eth_signature) = self.tx().await?;
        let tx_hashes = provider
            .send_txs_batch(vec![(tx_nft, None), (tx_fee, None)], eth_signature)
            .await?;

        Ok(tx_hashes)
    }

    /// Sets the transaction token id. Returns an error if token is not supported by zkSync.
    pub fn token(mut self, token: TokenId) -> Result<Self, ClientError> {
        if token.0 < MIN_NFT_TOKEN_ID {
            return Err(ClientError::UnknownToken);
        }
        self.token = Some(token);
        Ok(self)
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

    /// Sets the transaction recipient.
    pub fn to(mut self, to: Address) -> Self {
        self.to = Some(to);
        self
    }

    /// Sets the unix format timestamp of the first moment when transaction execution is valid.
    pub fn valid_from(mut self, valid_from: u64) -> Self {
        self.valid_from = Some(valid_from);
        self
    }

    /// Sets the unix format timestamp of the last moment when transaction execution is valid.
    pub fn valid_until(mut self, valid_until: u64) -> Self {
        self.valid_until = Some(valid_until);
        self
    }

    /// Same as `TransferNFTBuilder::to`, but accepts a string address value.
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
