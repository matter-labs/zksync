//! This module contains the definition of the fee token validator,
//! an entity which decides whether certain ERC20 token is suitable for paying fees.

// Built-in uses
use std::str::FromStr;
// Workspace uses
use zksync_types::{tokens::TokenLike, Address};
// Local uses
use crate::utils::token_db_cache::TokenDBCache;

/// Fee token validator decides whether certain ERC20 token is suitable for paying fees.
#[derive(Debug, Clone)]
pub struct FeeTokenValidator {
    tokens_cache: TokenDBCache,
}

impl FeeTokenValidator {
    /// Returns `true` if token can be used to pay fees.
    pub async fn token_supported(&self, token: TokenLike) -> anyhow::Result<bool> {
        // Currently we add tokens in zkSync manually, thus we can decide whether token is acceptable in before.
        // Later we'll check Uniswap trading volume for tokens.
        let not_supported_tokens = &[
            Address::from_str("0x38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7").unwrap(), // PHNX (PhoenixDAO)
        ];

        let token = self.tokens_cache.get_token(token).await?;
        if let Some(token) = token {
            let not_acceptable = not_supported_tokens.iter().any(|&t| t == token.address);
            Ok(!not_acceptable)
        } else {
            // Unknown tokens aren't suitable for our needs, obviously.
            Ok(false)
        }
    }
}
