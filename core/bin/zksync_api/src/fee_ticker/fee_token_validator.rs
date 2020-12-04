//! This module contains the definition of the fee token validator,
//! an entity which decides whether certain ERC20 token is suitable for paying fees.

// Built-in uses
use std::collections::{HashMap, HashSet};
// Workspace uses
use zksync_types::{
    tokens::{Token, TokenLike},
    Address,
};
// Local uses
use crate::utils::token_db_cache::TokenDBCache;

/// Fee token validator decides whether certain ERC20 token is suitable for paying fees.
#[derive(Debug, Clone)]
pub(crate) struct FeeTokenValidator {
    tokens_cache: TokenCacheWrapper,
    /// List of tokens that aren't accepted to pay fees in.
    disabled_tokens: HashSet<Address>,
}

impl FeeTokenValidator {
    pub(crate) fn new(
        cache: impl Into<TokenCacheWrapper>,
        disabled_tokens: HashSet<Address>,
    ) -> Self {
        Self {
            tokens_cache: cache.into(),
            disabled_tokens,
        }
    }

    /// Returns `true` if token can be used to pay fees.
    pub(crate) async fn token_allowed(&self, token: TokenLike) -> anyhow::Result<bool> {
        let token = self.resolve_token(token).await?;

        self.check_token(token).await
    }

    async fn resolve_token(&self, token: TokenLike) -> anyhow::Result<Option<Token>> {
        self.tokens_cache.get_token(token).await
    }

    async fn check_token(&self, token: Option<Token>) -> anyhow::Result<bool> {
        // Currently we add tokens in zkSync manually, thus we can decide whether token is acceptable in before.
        // Later we'll check Uniswap trading volume for tokens. That's why this function is already `async` even
        // though it's not really `async` at this moment.

        if let Some(token) = token {
            let not_acceptable = self.disabled_tokens.contains(&token.address);
            Ok(!not_acceptable)
        } else {
            // Unknown tokens aren't suitable for our needs, obviously.
            Ok(false)
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum TokenCacheWrapper {
    DB(TokenDBCache),
    Memory(HashMap<TokenLike, Token>),
}

impl From<TokenDBCache> for TokenCacheWrapper {
    fn from(cache: TokenDBCache) -> Self {
        Self::DB(cache)
    }
}

impl From<HashMap<TokenLike, Token>> for TokenCacheWrapper {
    fn from(cache: HashMap<TokenLike, Token>) -> Self {
        Self::Memory(cache)
    }
}

impl TokenCacheWrapper {
    pub async fn get_token(&self, token_like: TokenLike) -> anyhow::Result<Option<Token>> {
        match self {
            Self::DB(cache) => cache.get_token(token_like).await,
            Self::Memory(cache) => Ok(cache.get(&token_like).cloned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::str::FromStr;

    #[tokio::test]
    async fn check_tokens() {
        let dai_token_address =
            Address::from_str("6b175474e89094c44da98b954eedeac495271d0f").unwrap();
        let dai_token = Token::new(1, dai_token_address, "DAI", 18);
        let phnx_token_address =
            Address::from_str("38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7").unwrap();
        let phnx_token = Token::new(2, phnx_token_address, "PHNX", 18);

        let mut tokens = HashMap::new();
        tokens.insert(TokenLike::Address(dai_token_address), dai_token);
        tokens.insert(TokenLike::Address(phnx_token_address), phnx_token);

        let mut disabled_tokens = HashSet::new();
        disabled_tokens.insert(phnx_token_address);

        let validator = FeeTokenValidator::new(tokens, disabled_tokens);

        let dai_allowed = validator
            .token_allowed(TokenLike::Address(dai_token_address))
            .await
            .unwrap();
        let phnx_allowed = validator
            .token_allowed(TokenLike::Address(phnx_token_address))
            .await
            .unwrap();
        assert_eq!(dai_allowed, true);
        assert_eq!(phnx_allowed, false);
    }
}
