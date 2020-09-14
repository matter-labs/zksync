use models::node::{Token, TokenLike};
use std::collections::HashMap;

#[derive(Debug)]
pub struct TokensCache {
    tokens: HashMap<String, Token>,
}

impl TokensCache {
    pub fn new(tokens: HashMap<String, Token>) -> Self {
        Self { tokens }
    }

    pub fn resolve(&self, token: TokenLike) -> Option<Token> {
        match token {
            TokenLike::Symbol(symbol) => self.tokens.get(&symbol).cloned(),
            TokenLike::Address(address) => self
                .tokens
                .values()
                .find(|el| el.address == address)
                .cloned(),
            TokenLike::Id(id) => self.tokens.values().find(|el| el.id == id).cloned(),
        }
    }
}
