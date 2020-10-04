use std::collections::HashMap;
use zksync::{tokens_cache::TokensCache, web3::types::H160, zksync_types::Token};

#[test]
fn test_tokens_cache() {
    let mut tokens: HashMap<String, Token> = HashMap::default();

    let token_eth = Token::new(0, H160::default(), "ETH", 18);
    tokens.insert("ETH".to_string(), token_eth.clone());
    let token_dai = Token::new(1, H160::random(), "DAI", 18);
    tokens.insert("DAI".to_string(), token_dai.clone());

    let uncahed_token = Token::new(2, H160::random(), "UNC", 5);

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
