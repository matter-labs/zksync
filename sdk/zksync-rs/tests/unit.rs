use std::collections::HashMap;
use zksync::{tokens_cache::TokensCache, utils::*, web3::types::H160, zksync_types::Token};
use zksync_config::test_config::unit_vectors::{Config as TestVectorsConfig, TestEntry};
use zksync_crypto::PrivateKey;
use zksync_types::{tx::TxSignature, AccountId, Nonce, TokenId};

#[test]
fn test_tokens_cache() {
    let mut tokens: HashMap<String, Token> = HashMap::default();

    let token_eth = Token::new(TokenId(0), H160::default(), "ETH", 18);
    tokens.insert("ETH".to_string(), token_eth.clone());
    let token_dai = Token::new(TokenId(1), H160::random(), "DAI", 18);
    tokens.insert("DAI".to_string(), token_dai.clone());

    let uncahed_token = Token::new(TokenId(2), H160::random(), "UNC", 5);

    let tokens_hash = TokensCache::new(tokens);

    assert_eq!(
        tokens_hash.resolve(token_eth.address.into()),
        Some(token_eth.clone())
    );
    assert_eq!(
        tokens_hash.resolve(token_eth.id.into()),
        Some(token_eth.clone())
    );
    assert_eq!(
        tokens_hash.resolve((&token_eth.symbol as &str).into()),
        Some(token_eth.clone())
    );

    assert_eq!(
        tokens_hash.resolve(token_dai.address.into()),
        Some(token_dai.clone())
    );
    assert_eq!(
        tokens_hash.resolve(token_dai.id.into()),
        Some(token_dai.clone())
    );
    assert_eq!(
        tokens_hash.resolve((&token_dai.symbol as &str).into()),
        Some(token_dai.clone())
    );

    assert_eq!(tokens_hash.resolve(uncahed_token.address.into()), None);
    assert_eq!(tokens_hash.resolve(uncahed_token.id.into()), None);
    assert_eq!(
        tokens_hash.resolve((&uncahed_token.symbol as &str).into()),
        None
    );

    assert!(tokens_hash.is_eth(token_eth.address.into()));
    assert!(tokens_hash.is_eth(token_eth.id.into()));
    assert!(tokens_hash.is_eth((&token_eth.symbol as &str).into()));

    assert!(!tokens_hash.is_eth(token_dai.address.into()));
    assert!(!tokens_hash.is_eth(token_dai.id.into()));
    assert!(!tokens_hash.is_eth((&token_dai.symbol as &str).into()));
}

fn priv_key_from_raw(raw: &[u8]) -> Option<PrivateKey> {
    use zksync_crypto::{
        bellman::{pairing::ff::PrimeField, PrimeFieldRepr},
        franklin_crypto::alt_babyjubjub::fs::FsRepr,
        priv_key_from_fs, Fs,
    };

    let mut fs_repr = FsRepr::default();
    fs_repr.read_be(raw).ok()?;
    Fs::from_repr(fs_repr).ok().map(priv_key_from_fs)
}

fn assert_tx_signature(signature: &TxSignature, expected_pub: &str, expected_sig: &str) {
    let TxSignature { pub_key, signature } = signature;

    let pub_point = pub_key.serialize_packed().unwrap();
    assert_eq!(hex::encode(pub_point), expected_pub);

    let packed_sig = signature.serialize_packed().unwrap();
    assert_eq!(hex::encode(packed_sig), expected_sig);
}

#[cfg(test)]
mod primitives_with_vectors {
    use super::*;

    #[test]
    fn test_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.crypto_primitives.items {
            let private_key =
                private_key_from_seed(&inputs.seed).expect("Cannot get key from seed");

            assert_eq!(
                priv_key_from_raw(&outputs.private_key).unwrap().0,
                private_key.0
            );

            let signature = TxSignature::sign_musig(&private_key, &inputs.message);
            assert_tx_signature(&signature, &outputs.pub_key, &outputs.signature);
        }
    }
}

#[cfg(test)]
mod utils_with_vectors {
    use super::*;
    use zksync_utils::format_units;

    #[test]
    fn test_token_packing() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.utils.amount_packing.items {
            let token_amount = inputs.value;

            assert_eq!(is_token_amount_packable(&token_amount), outputs.packable);
            assert_eq!(
                closest_packable_token_amount(&token_amount),
                outputs.closest_packable
            );
            assert_eq!(pack_token_amount(&token_amount), outputs.packed_value);
        }
    }

    #[test]
    fn test_fee_packing() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.utils.fee_packing.items {
            let fee_amount = inputs.value;

            assert_eq!(is_fee_amount_packable(&fee_amount), outputs.packable);
            assert_eq!(
                closest_packable_fee_amount(&fee_amount),
                outputs.closest_packable
            );
            assert_eq!(pack_fee_amount(&fee_amount), outputs.packed_value);
        }
    }

    #[test]
    fn test_formatting() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.utils.token_formatting.items {
            let units_str = format_units(inputs.amount, inputs.decimals);
            assert_eq!(format!("{} {}", units_str, inputs.token), outputs.formatted);
        }
    }
}

#[cfg(test)]
mod signatures_with_vectors {
    use super::*;
    use zksync::{signer::Signer, WalletCredentials};
    use zksync_config::test_config::unit_vectors::TxData;
    use zksync_eth_signer::PrivateKeySigner;
    use zksync_types::tx::{ChangePubKeyECDSAData, ChangePubKeyEthAuthData};
    use zksync_types::{network::Network, AccountId, Address, H256};

    async fn get_signer(
        eth_private_key_raw: &[u8],
        from_address: Address,
        account_id: AccountId,
    ) -> Signer<PrivateKeySigner> {
        let eth_private_key = H256::from_slice(eth_private_key_raw);
        let eth_signer = PrivateKeySigner::new(eth_private_key);

        let creds = WalletCredentials::from_eth_signer(from_address, eth_signer, Network::Mainnet)
            .await
            .unwrap();

        let mut signer = Signer::with_credentials(creds);
        signer.set_account_id(Some(account_id));
        signer
    }

    #[tokio::test]
    async fn test_transfer_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let TxData::Transfer {
                data: transfer_tx,
                eth_sign_data: sign_data,
            } = &inputs.data
            {
                let signer = get_signer(
                    &inputs.eth_private_key,
                    transfer_tx.from,
                    sign_data.account_id,
                )
                .await;

                let token = Token {
                    id: transfer_tx.token_id,
                    address: Default::default(),
                    symbol: sign_data.string_token.clone(),
                    decimals: 0,
                    is_nft: false,
                };
                let (transfer, eth_signature) = signer
                    .sign_transfer(
                        token,
                        transfer_tx.amount.clone(),
                        transfer_tx.fee.clone(),
                        sign_data.to,
                        sign_data.nonce,
                        transfer_tx.time_range,
                    )
                    .await
                    .expect("Transfer signing error");

                assert_eq!(transfer.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &transfer.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );

                assert_eq!(
                    transfer
                        .get_ethereum_sign_message(&sign_data.string_token, 0)
                        .into_bytes(),
                    outputs.eth_sign_message.unwrap()
                );

                if let Some(expected_eth_signature) = outputs.eth_signature {
                    let eth_signature = eth_signature.unwrap().serialize_packed();
                    assert_eq!(&eth_signature[..], expected_eth_signature.as_slice());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_withdraw_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let TxData::Withdraw {
                data: withdraw_tx,
                eth_sign_data: sign_data,
            } = &inputs.data
            {
                let signer = get_signer(
                    &inputs.eth_private_key,
                    withdraw_tx.from,
                    sign_data.account_id,
                )
                .await;

                let token = Token {
                    id: withdraw_tx.token_id,
                    address: Default::default(),
                    symbol: sign_data.string_token.clone(),
                    decimals: 0,
                    is_nft: false,
                };
                let (withdraw, eth_signature) = signer
                    .sign_withdraw(
                        token,
                        withdraw_tx.amount.clone(),
                        withdraw_tx.fee.clone(),
                        sign_data.eth_address,
                        sign_data.nonce,
                        withdraw_tx.time_range,
                    )
                    .await
                    .expect("Withdraw signing error");

                assert_eq!(withdraw.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &withdraw.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );

                assert_eq!(
                    withdraw
                        .get_ethereum_sign_message(&sign_data.string_token, 0)
                        .into_bytes(),
                    outputs.eth_sign_message.unwrap()
                );

                if let Some(expected_eth_signature) = outputs.eth_signature {
                    let eth_signature = eth_signature.unwrap().serialize_packed();
                    assert_eq!(&eth_signature[..], expected_eth_signature.as_slice());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_withdraw_nft_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let TxData::WithdrawNFT {
                data: withdraw_nft_tx,
                eth_sign_data: sign_data,
            } = &inputs.data
            {
                let signer = get_signer(
                    &inputs.eth_private_key,
                    withdraw_nft_tx.from,
                    withdraw_nft_tx.account_id,
                )
                .await;

                let fee_token = Token {
                    id: withdraw_nft_tx.fee_token_id,
                    address: Default::default(),
                    symbol: sign_data.string_fee_token.clone(),
                    decimals: 0,
                    is_nft: false,
                };

                let (withdraw_nft, eth_signature) = signer
                    .sign_withdraw_nft(
                        withdraw_nft_tx.to,
                        withdraw_nft_tx.token_id,
                        fee_token,
                        withdraw_nft_tx.fee.clone(),
                        withdraw_nft_tx.nonce,
                        withdraw_nft_tx.time_range,
                    )
                    .await
                    .expect("Withdraw nft signing error");

                assert_eq!(withdraw_nft.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &withdraw_nft.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );

                assert_eq!(
                    withdraw_nft
                        .get_ethereum_sign_message(&sign_data.string_fee_token, 0)
                        .into_bytes(),
                    outputs.eth_sign_message.unwrap()
                );

                if let Some(expected_eth_signature) = outputs.eth_signature {
                    let eth_signature = eth_signature.unwrap().serialize_packed();
                    assert_eq!(&eth_signature[..], expected_eth_signature.as_slice());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_mint_nft_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let TxData::MintNFT {
                data: mint_nft_tx,
                eth_sign_data: sign_data,
            } = &inputs.data
            {
                let signer = get_signer(
                    &inputs.eth_private_key,
                    mint_nft_tx.creator_address,
                    mint_nft_tx.creator_id,
                )
                .await;

                let fee_token = Token {
                    id: mint_nft_tx.fee_token_id,
                    address: Default::default(),
                    symbol: sign_data.string_fee_token.clone(),
                    decimals: 0,
                    is_nft: false,
                };

                let (mint_nft, eth_signature) = signer
                    .sign_mint_nft(
                        mint_nft_tx.recipient,
                        mint_nft_tx.content_hash,
                        fee_token,
                        mint_nft_tx.fee.clone(),
                        mint_nft_tx.nonce,
                    )
                    .await
                    .expect("Withdraw nft signing error");

                assert_eq!(mint_nft.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &mint_nft.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );

                assert_eq!(
                    mint_nft
                        .get_ethereum_sign_message(&sign_data.string_fee_token, 0)
                        .into_bytes(),
                    outputs.eth_sign_message.unwrap()
                );

                if let Some(expected_eth_signature) = outputs.eth_signature {
                    let eth_signature = eth_signature.unwrap().serialize_packed();
                    assert_eq!(&eth_signature[..], expected_eth_signature.as_slice());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_change_pubkey_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let TxData::ChangePubKey {
                data: change_pubkey_tx,
                eth_sign_data: sign_data,
            } = &inputs.data
            {
                let mut signer = get_signer(
                    &inputs.eth_private_key,
                    change_pubkey_tx.account,
                    sign_data.account_id,
                )
                .await;
                signer.pubkey_hash = change_pubkey_tx.new_pk_hash;

                let token = Token {
                    id: change_pubkey_tx.fee_token_id,
                    address: Default::default(),
                    symbol: String::new(),
                    decimals: 0,
                    is_nft: false,
                };
                let change_pub_key = signer
                    .sign_change_pubkey_tx(
                        sign_data.nonce,
                        false,
                        token,
                        change_pubkey_tx.fee.clone(),
                        change_pubkey_tx.time_range,
                    )
                    .await
                    .expect("Change pub key signing error");

                assert_eq!(change_pub_key.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &change_pub_key.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );

                assert_eq!(
                    change_pub_key.get_eth_signed_data().unwrap(),
                    outputs.eth_sign_message.unwrap()
                );

                if let Some(expected_eth_signature) = outputs.eth_signature {
                    let eth_signature = match &change_pub_key.eth_auth_data {
                        Some(ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData {
                            eth_signature,
                            ..
                        })) => eth_signature.serialize_packed(),
                        _ => panic!("No ChangePubKey ethereum siganture"),
                    };
                    assert_eq!(&eth_signature[..], expected_eth_signature.as_slice());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_forced_exit_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let TxData::ForcedExit { data: forced_exit } = &inputs.data {
                let signer = get_signer(
                    &inputs.eth_private_key,
                    forced_exit.from,
                    forced_exit.initiator_account_id,
                )
                .await;

                let token = Token {
                    id: forced_exit.token_id,
                    address: Default::default(),
                    symbol: String::new(),
                    decimals: 0,
                    is_nft: false,
                };
                let (forced_exit, _) = signer
                    .sign_forced_exit(
                        forced_exit.target,
                        token,
                        forced_exit.fee.clone(),
                        forced_exit.nonce,
                        forced_exit.time_range,
                    )
                    .await
                    .expect("Forced exit signing error");

                assert_eq!(forced_exit.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &forced_exit.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );
            }
        }
    }
}

#[cfg(test)]
mod wallet_tests {
    use super::*;
    use num::{BigUint, ToPrimitive};
    use zksync::{
        error::ClientError,
        provider::Provider,
        signer::Signer,
        types::{
            AccountInfo, AccountState, BlockStatus, ContractAddress, EthOpInfo, Fee, Tokens,
            TransactionInfo,
        },
        Network, Wallet, WalletCredentials,
    };
    use zksync_eth_signer::PrivateKeySigner;
    use zksync_types::{
        tokens::get_genesis_token_list,
        tx::{PackedEthSignature, TxHash},
        Address, PubKeyHash, TokenId, TokenLike, TxFeeTypes, ZkSyncTx, H256,
    };

    #[derive(Debug, Clone)]
    /// Provides some hardcoded values the `Provider` responsible to
    /// without communicating with the network
    struct MockProvider {
        network: Network,
        eth_private_key: H256,
    }

    impl MockProvider {
        async fn pub_key_hash(&self) -> PubKeyHash {
            let address =
                PackedEthSignature::address_from_private_key(&self.eth_private_key).unwrap();
            let eth_signer = PrivateKeySigner::new(self.eth_private_key);
            let creds = WalletCredentials::from_eth_signer(address, eth_signer, self.network)
                .await
                .unwrap();
            let signer = Signer::with_credentials(creds);
            signer.pubkey_hash
        }
    }

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        /// Returns the example `AccountInfo` instance:
        ///  - assigns the '42' value to account_id;
        ///  - assigns the PubKeyHash to match the wallet's signer's PubKeyHash
        ///  - adds single entry of "DAI" token to the committed balances;
        ///  - adds single entry of "USDC" token to the verified balances.
        async fn account_info(&self, address: Address) -> Result<AccountInfo, ClientError> {
            let mut committed_balances = HashMap::new();
            committed_balances.insert("DAI".into(), BigUint::from(12345_u32).into());

            let mut verified_balances = HashMap::new();
            verified_balances.insert("USDC".into(), BigUint::from(98765_u32).into());

            Ok(AccountInfo {
                address,
                id: Some(AccountId(42)),
                depositing: Default::default(),
                committed: AccountState {
                    balances: committed_balances,
                    nonce: Nonce(0),
                    pub_key_hash: self.pub_key_hash().await,
                    ..Default::default()
                },
                verified: AccountState {
                    balances: verified_balances,
                    ..Default::default()
                },
            })
        }

        /// Returns first three tokens from the configuration found in
        /// $ZKSYNC_HOME/etc/tokens/<NETWORK>.json
        async fn tokens(&self) -> Result<Tokens, ClientError> {
            let genesis_tokens = get_genesis_token_list(&self.network.to_string())
                .expect("Initial token list not found");

            let tokens = (1..)
                .zip(&genesis_tokens[..3])
                .map(|(id, token)| Token {
                    id: TokenId(id),
                    symbol: token.symbol.clone(),
                    address: token.address,
                    decimals: token.decimals,
                    is_nft: false,
                })
                .map(|token| (token.symbol.clone(), token))
                .collect();
            Ok(tokens)
        }

        async fn tx_info(&self, _tx_hash: TxHash) -> Result<TransactionInfo, ClientError> {
            unreachable!()
        }

        async fn get_tx_fee(
            &self,
            _tx_type: TxFeeTypes,
            _address: Address,
            _token: impl Into<TokenLike> + Send + 'async_trait,
        ) -> Result<Fee, ClientError> {
            unreachable!()
        }

        async fn get_txs_batch_fee(
            &self,
            _tx_types: Vec<TxFeeTypes>,
            _addresses: Vec<Address>,
            _token: impl Into<TokenLike> + Send + 'async_trait,
        ) -> Result<BigUint, ClientError> {
            unreachable!()
        }

        async fn ethop_info(&self, _serial_id: u32) -> Result<EthOpInfo, ClientError> {
            unreachable!()
        }

        async fn get_eth_tx_for_withdrawal(
            &self,
            _withdrawal_hash: TxHash,
        ) -> Result<Option<String>, ClientError> {
            unreachable!()
        }

        /// Returns the example `ContractAddress` instance:
        ///  - the HEX-encoded sequence of bytes [0..20) provided as the `main_contract`;
        ///  - the `gov_contract` is not usable in tests and it is simply an empty string.
        async fn contract_address(&self) -> Result<ContractAddress, ClientError> {
            Ok(ContractAddress {
                main_contract: "0x000102030405060708090a0b0c0d0e0f10111213".to_string(),
                gov_contract: "".to_string(),
            })
        }

        async fn send_tx(
            &self,
            _tx: ZkSyncTx,
            _eth_signature: Option<PackedEthSignature>,
        ) -> Result<TxHash, ClientError> {
            unreachable!()
        }

        async fn send_txs_batch(
            &self,
            _txs_signed: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
            _eth_signature: Option<PackedEthSignature>,
        ) -> Result<Vec<TxHash>, ClientError> {
            unreachable!()
        }

        fn network(&self) -> Network {
            self.network
        }
    }

    async fn get_test_wallet(
        private_key_raw: &[u8],
        network: Network,
    ) -> Wallet<PrivateKeySigner, MockProvider> {
        let private_key = H256::from_slice(private_key_raw);
        let address = PackedEthSignature::address_from_private_key(&private_key).unwrap();

        let eth_signer = PrivateKeySigner::new(private_key);
        let creds = WalletCredentials::from_eth_signer(address, eth_signer, Network::Mainnet)
            .await
            .unwrap();

        let provider = MockProvider {
            network,
            eth_private_key: private_key,
        };
        Wallet::new(provider, creds).await.unwrap()
    }

    #[tokio::test]
    async fn test_wallet_address() {
        let wallet = get_test_wallet(&[5; 32], Network::Mainnet).await;
        let expected_address =
            PackedEthSignature::address_from_private_key(&H256::from([5; 32])).unwrap();
        assert_eq!(wallet.address(), expected_address);
    }

    #[tokio::test]
    async fn test_wallet_account_info() {
        let wallet = get_test_wallet(&[10; 32], Network::Mainnet).await;
        let account_info = wallet.account_info().await.unwrap();
        assert_eq!(account_info.address, wallet.address());
    }

    #[tokio::test]
    async fn test_wallet_account_id() {
        let wallet = get_test_wallet(&[14; 32], Network::Mainnet).await;
        assert_eq!(wallet.account_id(), Some(AccountId(42)));
    }

    #[tokio::test]
    async fn test_wallet_refresh_tokens() {
        let mut wallet = get_test_wallet(&[20; 32], Network::Mainnet).await;
        let _dai_token = wallet
            .tokens
            .resolve(TokenLike::Symbol("DAI".into()))
            .unwrap();

        wallet.provider.network = Network::Rinkeby;
        wallet.refresh_tokens_cache().await.unwrap();

        // DAI is not in the Rinkeby network
        assert!(wallet
            .tokens
            .resolve(TokenLike::Symbol("DAI".into()))
            .is_none());
    }

    #[tokio::test]
    async fn test_wallet_get_balance_committed() {
        let wallet = get_test_wallet(&[40; 32], Network::Mainnet).await;
        let balance = wallet
            .get_balance(BlockStatus::Committed, "DAI")
            .await
            .unwrap();
        assert_eq!(balance.to_u32(), Some(12345));
    }

    #[tokio::test]
    async fn test_wallet_get_balance_committed_not_existent() {
        let wallet = get_test_wallet(&[40; 32], Network::Mainnet).await;
        let result = wallet.get_balance(BlockStatus::Committed, "ETH").await;

        assert_eq!(result.unwrap_err(), ClientError::UnknownToken);
    }

    #[tokio::test]
    async fn test_wallet_get_balance_verified() {
        let wallet = get_test_wallet(&[50; 32], Network::Mainnet).await;
        let balance = wallet
            .get_balance(BlockStatus::Verified, "USDC")
            .await
            .unwrap();
        assert_eq!(balance.to_u32(), Some(98765));
    }

    #[tokio::test]
    async fn test_wallet_get_balance_verified_not_existent() {
        let wallet = get_test_wallet(&[50; 32], Network::Mainnet).await;
        let result = wallet.get_balance(BlockStatus::Verified, "ETH").await;

        assert_eq!(result.unwrap_err(), ClientError::UnknownToken);
    }

    #[tokio::test]
    async fn test_wallet_is_signing_key_set() {
        let wallet = get_test_wallet(&[50; 32], Network::Mainnet).await;
        assert!(wallet.is_signing_key_set().await.unwrap());
    }

    #[tokio::test]
    async fn test_wallet_ethereum() {
        let wallet = get_test_wallet(&[50; 32], Network::Mainnet).await;
        let eth_provider = wallet.ethereum("http://some.random.url").await.unwrap();
        let expected_address: Vec<_> = (0..20).collect();
        assert_eq!(eth_provider.contract_address().as_bytes(), expected_address);
    }
}
