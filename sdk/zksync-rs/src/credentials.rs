use crate::{error::ClientError, types::network::Network, utils::private_key_from_seed};

use web3::types::{Address, H256};
use zksync_crypto::PrivateKey;
use zksync_eth_signer::{EthereumSigner, PrivateKeySigner};
use zksync_types::tx::TxEthSignature;

pub struct WalletCredentials<S: EthereumSigner> {
    pub(crate) eth_signer: Option<S>,
    pub(crate) eth_address: Address,
    pub(crate) zksync_private_key: PrivateKey,
}

impl<S: EthereumSigner> std::fmt::Debug for WalletCredentials<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalletCredentials")
            .field("eth_address", &self.eth_address)
            .finish()
    }
}

impl<S: EthereumSigner> WalletCredentials<S> {
    /// Creates wallet credentials from the provided Ethereum wallet private key.
    ///
    /// ## Arguments
    ///
    /// - `eth_address`: Address of the corresponding Ethereum wallet.
    /// - `eth_signer`: Abstract signer that signs messages and transactions.
    /// - `network`: Network this wallet is used on.
    pub async fn from_eth_signer(
        eth_address: Address,
        eth_signer: S,
        network: Network,
    ) -> Result<Self, ClientError> {
        // Pre-defined message to generate seed from.
        const MESSAGE: &str =
            "Access zkSync account.\n\nOnly sign this message for a trusted client!";

        // Add chain_id to the message to prevent replay attacks between networks
        // This is added for testnets only
        let eth_sign_message = if let Network::Mainnet = network {
            MESSAGE.into()
        } else {
            format!("{}\nChain ID: {}.", MESSAGE, network.chain_id())
        }
        .into_bytes();

        let signature = eth_signer
            .sign_message(&eth_sign_message)
            .await
            .map_err(|_| ClientError::IncorrectCredentials)?;

        let packed_signature =
            if let TxEthSignature::EthereumSignature(packed_signature) = signature {
                packed_signature
            } else {
                return Err(ClientError::IncorrectCredentials);
            };

        // Check that signature is correct and corresponds to the provided address.
        let address_from_sig = packed_signature.signature_recover_signer(&eth_sign_message);
        if !address_from_sig
            .map(|address_from_pk| eth_address == address_from_pk)
            .unwrap_or(false)
        {
            return Err(ClientError::IncorrectCredentials);
        }

        // Generate seed, and then zkSync private key.
        let signature_bytes = packed_signature.serialize_packed();
        let zksync_pk = private_key_from_seed(&signature_bytes)?;

        Ok(Self {
            eth_signer: Some(eth_signer),
            eth_address,
            zksync_private_key: zksync_pk,
        })
    }

    /// Creates wallet credentials from the provided seed.
    /// zkSync private key will be randomly generated and Ethereum private key will be not set.
    /// Wallet created with such credentials won't be capable of performing on-chain operations,
    /// such as deposits and full exits.
    ///
    /// ## Arguments
    ///
    /// - `eth_address`: Address of the corresponding Ethereum wallet.
    /// - `seed`: A random bytearray to generate private key from. Must be at least 32 bytes long.
    pub fn from_seed(eth_address: Address, seed: &[u8]) -> Result<Self, ClientError> {
        let zksync_pk = private_key_from_seed(seed)?;

        Ok(Self {
            eth_signer: None,
            eth_address,
            zksync_private_key: zksync_pk,
        })
    }

    /// Creates wallet credentials from the provided keys.
    ///
    /// ## Arguments
    ///
    /// - `eth_address`: Address of the corresponding Ethereum wallet.
    /// - `private_key`: Private key of a zkSync account.
    /// - `eth_private_key`: Private key of a corresponding Ethereum wallet. If not set, on-chain operations won't be allowed for Wallet.
    pub fn from_pk(
        eth_address: Address,
        private_key: PrivateKey,
        eth_private_key: Option<H256>,
    ) -> WalletCredentials<PrivateKeySigner> {
        let eth_signer = eth_private_key.map(PrivateKeySigner::new);

        WalletCredentials {
            eth_address,
            eth_signer,
            zksync_private_key: private_key,
        }
    }
}
