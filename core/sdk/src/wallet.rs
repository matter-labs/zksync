use models::node::Address;

use crate::{
    credentials::WalletCredentials, error::ClientError, operations::*, provider::Provider,
    signer::Signer, tokens_cache::TokensCache,
};

#[derive(Debug)]
pub struct Wallet {
    pub provider: Provider,
    pub signer: Signer,
    pub tokens: TokensCache,
}

impl Wallet {
    pub async fn new(
        provider: Provider,
        credentials: WalletCredentials,
    ) -> Result<Self, ClientError> {
        let mut signer = Signer::new(
            credentials.zksync_private_key,
            credentials.eth_address,
            credentials.eth_private_key,
        );

        let account_info = provider.account_info(credentials.eth_address).await?;
        signer.set_account_id(account_info.id);

        let tokens = TokensCache::new(provider.tokens().await?);

        Ok(Wallet {
            provider,
            signer,
            tokens,
        })
    }

    /// Returns the wallet address.
    pub fn address(&self) -> Address {
        self.signer.address
    }

    /// Updates the list of tokens supported by zkSync.
    /// This method only needs to be called if a new token was added to zkSync after
    /// `Wallet` object was created.
    pub async fn refresh_tokens_cache(&mut self) -> Result<(), ClientError> {
        self.tokens = TokensCache::new(self.provider.tokens().await?);

        Ok(())
    }

    /// Returns `true` if signing key for account was set in zkSync network.
    /// In other words, returns `true` if `ChangePubKey` operation was performed for the
    /// account.
    ///
    /// If this method has returned `false`, one must send a `ChangePubKey` transaction
    /// via `Wallet::start_change_pubkey` method.
    pub fn is_signing_key_set(&self) -> bool {
        self.signer.get_account_id().is_some()
    }

    /// Initializes `Transfer` transaction sending.
    pub fn start_transfer(&self) -> TransferBuilder<'_> {
        TransferBuilder::new(self)
    }

    /// Initializes `ChangePubKey` transaction sending.
    pub fn start_change_pubkey(&self) -> ChangePubKeyBuilder<'_> {
        ChangePubKeyBuilder::new(self)
    }

    /// Initializes `Withdraw` transaction sending.
    pub fn start_withdraw(&self) -> WithdrawBuilder<'_> {
        WithdrawBuilder::new(self)
    }
}
