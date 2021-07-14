use num::BigUint;
use zksync_eth_signer::EthereumSigner;
use zksync_types::{
    helpers::{closest_packable_fee_amount, is_fee_amount_packable},
    tokens::TxFeeTypes,
    tx::PackedEthSignature,
    Address, Nonce, Token, TokenLike, ZkSyncTx, H256,
};

use crate::{
    error::ClientError, operations::SyncTransactionHandle, provider::Provider, wallet::Wallet,
};

#[derive(Debug)]
pub struct MintNFTBuilder<'a, S: EthereumSigner, P: Provider> {
    wallet: &'a Wallet<S, P>,
    recipient: Option<Address>,
    content_hash: Option<H256>,
    fee_token: Option<Token>,
    fee: Option<BigUint>,
    nonce: Option<Nonce>,
}

impl<'a, S, P> MintNFTBuilder<'a, S, P>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    /// Initializes a mint nft transaction building process.
    pub fn new(wallet: &'a Wallet<S, P>) -> Self {
        Self {
            wallet,
            recipient: None,
            content_hash: None,
            fee_token: None,
            fee: None,
            nonce: None,
        }
    }

    /// Directly returns the signed mint nft transaction for the subsequent usage.
    pub async fn tx(self) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let recipient = self
            .recipient
            .ok_or_else(|| ClientError::MissingRequiredField("recipient".into()))?;
        let content_hash = self
            .content_hash
            .ok_or_else(|| ClientError::MissingRequiredField("content_hash".into()))?;
        let fee_token = self
            .fee_token
            .ok_or_else(|| ClientError::MissingRequiredField("fee_token".into()))?;

        let fee = match self.fee {
            Some(fee) => fee,
            None => {
                let fee = self
                    .wallet
                    .provider
                    .get_tx_fee(TxFeeTypes::MintNFT, recipient, fee_token.id)
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

        self.wallet
            .signer
            .sign_mint_nft(recipient, content_hash, fee_token, fee, nonce)
            .await
            .map(|(tx, signature)| (ZkSyncTx::MintNFT(Box::new(tx)), signature))
            .map_err(ClientError::SigningError)
    }

    /// Sends the transaction, returning the handle for its awaiting.
    pub async fn send(self) -> Result<SyncTransactionHandle<P>, ClientError> {
        let provider = self.wallet.provider.clone();

        let (tx, eth_signature) = self.tx().await?;
        let tx_hash = provider.send_tx(tx, eth_signature).await?;

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
    pub fn recipient(mut self, recipient: Address) -> Self {
        self.recipient = Some(recipient);
        self
    }

    /// Same as `MintNFTBuilder::recipient`, but accepts a string address value.
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

    /// Sets the transaction nonce.
    pub fn nonce(mut self, nonce: Nonce) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Sets the transaction content hash.
    pub fn content_hash(mut self, content_hash: H256) -> Self {
        self.content_hash = Some(content_hash);
        self
    }
}
