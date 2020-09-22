use crate::{error::ClientError, types::Network, utils::private_key_from_seed};
use models::node::{tx::PackedEthSignature, PrivateKey};
use web3::types::{Address, H256};

pub struct WalletCredentials {
    pub(crate) eth_private_key: Option<H256>,
    pub(crate) eth_address: Address,
    pub(crate) zksync_private_key: PrivateKey,
}

impl std::fmt::Debug for WalletCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalletCredentials")
            .field("eth_address", &self.eth_address)
            .finish()
    }
}

impl WalletCredentials {
    /// Creates wallet credentials from the provided Ethereum wallet private key.
    ///
    /// ## Arguments
    ///
    /// - `eth_address`: Address of the corresponding Ethereum wallet.
    /// - `eth_private_key`: Private key of a corresponding Ethereum account.
    /// - `network`: Network this wallet is used on.
    pub fn from_eth_pk(
        eth_address: Address,
        eth_private_key: H256,
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
            format!("{}\nChainID: {}.", MESSAGE, network.chain_id())
        }
        .into_bytes();

        // Check that private key is correct and corresponds to the provided address.
        let address_from_pk = PackedEthSignature::address_from_private_key(&eth_private_key);
        if !address_from_pk
            .map(|address_from_pk| eth_address == address_from_pk)
            .unwrap_or(false)
        {
            return Err(ClientError::IncorrectCredentials);
        }

        // Generate seed, and then zkSync private key.
        let signature = PackedEthSignature::sign(&eth_private_key, &eth_sign_message)
            .map_err(|_| ClientError::IncorrectCredentials)?;

        let signature_bytes = signature.serialize_packed();
        let zksync_pk = private_key_from_seed(&signature_bytes)?;

        Ok(Self {
            eth_private_key: Some(eth_private_key),
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
            eth_private_key: None,
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
    ) -> Self {
        Self {
            eth_address,
            eth_private_key,
            zksync_private_key: private_key,
        }
    }
}
