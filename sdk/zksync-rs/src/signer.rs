// Built-in imports
use std::fmt;
use zksync_eth_signer::error::SignerError;
use zksync_eth_signer::EthereumSigner;
use zksync_types::tx::TxEthSignature;
// External uses
use num::BigUint;
// Workspace uses
use zksync_crypto::PrivateKey;
use zksync_types::tx::{ChangePubKey, PackedEthSignature};
use zksync_types::{AccountId, Address, Nonce, PubKeyHash, Token, Transfer, Withdraw};

fn signing_failed_error(err: impl ToString) -> SignerError {
    SignerError::SigningFailed(err.to_string())
}

pub struct Signer<S: EthereumSigner> {
    pub pubkey_hash: PubKeyHash,
    pub address: Address,
    pub(crate) private_key: PrivateKey,
    pub(crate) eth_signer: Option<S>,
    pub(crate) account_id: Option<AccountId>,
}

impl<S: EthereumSigner> fmt::Debug for Signer<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pk_contents = Vec::new();
        self.private_key
            .write(&mut pk_contents)
            .expect("Failed writing the private key contents");
        f.debug_struct("Signer")
            .field("pubkey_hash", &self.pubkey_hash)
            .field("address", &self.address)
            .finish()
    }
}

impl<S: EthereumSigner> Signer<S> {
    pub fn new(private_key: PrivateKey, address: Address, eth_signer: Option<S>) -> Self {
        let pubkey_hash = PubKeyHash::from_privkey(&private_key);

        Self {
            private_key,
            pubkey_hash,
            address,
            eth_signer,
            account_id: None,
        }
    }

    pub fn pubkey_hash(&self) -> &PubKeyHash {
        &self.pubkey_hash
    }

    pub fn set_account_id(&mut self, account_id: Option<AccountId>) {
        self.account_id = account_id;
    }

    pub fn get_account_id(&self) -> Option<AccountId> {
        self.account_id
    }

    pub async fn sign_change_pubkey_tx(
        &self,
        nonce: Nonce,
        auth_onchain: bool,
        fee_token: Token,
        fee: BigUint,
    ) -> Result<ChangePubKey, SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let mut change_pubkey = ChangePubKey::new_signed(
            account_id,
            self.address,
            self.pubkey_hash.clone(),
            fee_token.id,
            fee,
            nonce,
            None,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = if auth_onchain {
            None
        } else {
            let eth_signer = self
                .eth_signer
                .as_ref()
                .ok_or(SignerError::MissingEthSigner)?;

            let sign_bytes = change_pubkey
                .get_eth_signed_data()
                .map_err(signing_failed_error)?;
            let eth_signature = eth_signer
                .sign_message(&sign_bytes)
                .await
                .map_err(signing_failed_error)?;

            match eth_signature {
                TxEthSignature::EthereumSignature(packed_signature) => Some(packed_signature),
                _ => None,
            }
        };

        change_pubkey.eth_signature = eth_signature;

        if !auth_onchain {
            assert!(
                change_pubkey.verify_eth_signature() == Some(self.address),
                "eth signature is incorrect"
            );
        }

        Ok(change_pubkey)
    }

    pub async fn sign_transfer(
        &self,
        token: Token,
        amount: BigUint,
        fee: BigUint,
        to: Address,
        nonce: Nonce,
    ) -> Result<(Transfer, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let transfer = Transfer::new_signed(
            account_id,
            self.address,
            to,
            token.id,
            amount,
            fee,
            nonce,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message = transfer.get_ethereum_sign_message(&token.symbol, token.decimals);
                let signature = signer.sign_message(&message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((transfer, eth_signature))
    }

    pub async fn sign_withdraw(
        &self,
        token: Token,
        amount: BigUint,
        fee: BigUint,
        eth_address: Address,
        nonce: Nonce,
    ) -> Result<(Withdraw, Option<PackedEthSignature>), SignerError> {
        let account_id = self.account_id.ok_or(SignerError::NoSigningKey)?;

        let withdraw = Withdraw::new_signed(
            account_id,
            self.address,
            eth_address,
            token.id,
            amount,
            fee,
            nonce,
            &self.private_key,
        )
        .map_err(signing_failed_error)?;

        let eth_signature = match &self.eth_signer {
            Some(signer) => {
                let message = withdraw.get_ethereum_sign_message(&token.symbol, token.decimals);
                let signature = signer.sign_message(&message.as_bytes()).await?;

                if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                    Some(packed_signature)
                } else {
                    return Err(SignerError::MissingEthSigner);
                }
            }
            _ => None,
        };

        Ok((withdraw, eth_signature))
    }
}
