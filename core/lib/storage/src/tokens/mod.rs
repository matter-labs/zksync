// Built-in deps
use std::collections::HashMap;
use std::time::Instant;
// External imports
use num::{rational::Ratio, BigUint};

use thiserror::Error;
// Workspace imports
use zksync_types::{
    tokens::ApiNFT, AccountId, Address, Token, TokenId, TokenLike, TokenPrice, NFT,
};
use zksync_utils::ratio_to_big_decimal;
// Local imports
use self::records::{
    DBMarketVolume, DbTickerPrice, DbToken, StorageApiNFT, StorageNFT, StorageNFTCreator,
    StorageNFTFactory,
};

use crate::utils::address_to_stored_string;
use crate::{QueryResult, StorageProcessor};
use zksync_types::tokens::TokenMarketVolume;

pub mod records;

/// Precision of the USD price per token
pub(crate) const STORED_USD_PRICE_PRECISION: usize = 6;

/// Tokens schema handles the `tokens` table, providing methods to
/// get and store new tokens.
#[derive(Debug)]
pub struct TokensSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

#[derive(Debug, Error)]
pub enum StoreTokenError {
    #[error("{0}")]
    TokenAlreadyExistsError(String),
    #[error("{0}")]
    Other(anyhow::Error),
}

impl<'a, 'c> TokensSchema<'a, 'c> {
    /// Persists the new token in the database.
    pub async fn store_token(&mut self, token: Token) -> Result<(), StoreTokenError> {
        let start = Instant::now();

        let token_from_db: Option<Token> = sqlx::query_as!(
            DbToken,
            r#"
            SELECT * FROM tokens
            WHERE id = $1 OR address = $2 OR symbol = $3
            LIMIT 1
            "#,
            *token.id as i32,
            address_to_stored_string(&token.address),
            token.symbol,
        )
        .fetch_optional(self.0.conn())
        .await
        .map_err(|err| StoreTokenError::Other(err.into()))?
        .map(|db_token| db_token.into());

        if let Some(token_from_db) = token_from_db {
            let mut matched_parameters = Vec::new();

            if token_from_db.id == token.id {
                matched_parameters.push(format!("id = {}", token.id));
            }
            if token_from_db.symbol == token.symbol {
                matched_parameters.push(format!("symbol = {}", token.symbol));
            }
            if token_from_db.address == token.address {
                matched_parameters.push(format!("address = {}", token.address));
            }

            let error_message = format!(
                "tokens with such parameters already exist: {:#?}",
                matched_parameters
            );

            return Err(StoreTokenError::TokenAlreadyExistsError(error_message));
        }

        sqlx::query!(
            r#"
            INSERT INTO tokens ( id, address, symbol, decimals, is_nft )
            VALUES ( $1, $2, $3, $4, $5 )
            "#,
            token.id.0 as i32,
            address_to_stored_string(&token.address),
            token.symbol,
            i16::from(token.decimals),
            token.is_nft
        )
        .execute(self.0.conn())
        .await
        .map_err(|err| StoreTokenError::Other(err.into()))?;

        metrics::histogram!("sql.token.store_token", start.elapsed());
        Ok(())
    }

    /// If a token with a given ID exists, then it replaces the information about the
    /// token with a new one, otherwise, saves the token.
    pub async fn store_or_update_token(&mut self, token: Token) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            r#"
            INSERT INTO tokens ( id, address, symbol, decimals, is_nft )
            VALUES ( $1, $2, $3, $4, $5 )
            ON CONFLICT (id)
            DO
              UPDATE SET address = $2, symbol = $3, decimals = $4
            "#,
            *token.id as i32,
            address_to_stored_string(&token.address),
            token.symbol,
            i16::from(token.decimals),
            token.is_nft
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.store_token", start.elapsed());
        Ok(())
    }

    /// Loads all the stored tokens from the database.
    /// Alongside with the tokens added via `store_token` method, the default `ETH` token
    /// is returned.
    pub async fn load_tokens(&mut self) -> QueryResult<HashMap<TokenId, Token>> {
        let start = Instant::now();
        let tokens = sqlx::query_as!(
            DbToken,
            r#"
            SELECT * FROM tokens WHERE is_nft = false
            ORDER BY id ASC
            "#,
        )
        .fetch_all(self.0.conn())
        .await?;

        let result = tokens
            .into_iter()
            .map(|t| {
                let token: Token = t.into();
                (token.id, token)
            })
            .collect();

        metrics::histogram!("sql.token.load_tokens", start.elapsed());
        Ok(result)
    }

    /// Loads all the stored tokens, which have market_volume (ticker_market_volume table)
    /// not less than parameter (min_market_volume)
    pub async fn load_tokens_by_market_volume(
        &mut self,
        min_market_volume: Ratio<BigUint>,
    ) -> QueryResult<HashMap<TokenId, Token>> {
        let start = Instant::now();
        let tokens = sqlx::query_as!(
            DbToken,
            r#"
            SELECT id, address, symbol, decimals, is_nft
            FROM tokens
            INNER JOIN ticker_market_volume
            ON tokens.id = ticker_market_volume.token_id
            WHERE ticker_market_volume.market_volume >= $1
            AND is_nft = false
            ORDER BY id ASC
            "#,
            ratio_to_big_decimal(&min_market_volume, STORED_USD_PRICE_PRECISION)
        )
        .fetch_all(self.0.conn())
        .await?;

        let result = Ok(tokens
            .into_iter()
            .map(|t| {
                let token: Token = t.into();
                (token.id, token)
            })
            .collect());

        metrics::histogram!("sql.token.load_tokens_by_market_volume", start.elapsed());
        result
    }

    /// Gets the last used token ID from Database.
    pub async fn get_last_token_id(&mut self) -> QueryResult<TokenId> {
        let start = Instant::now();
        let last_token_id = sqlx::query!(
            r#"
            SELECT max(id) as "id!" FROM tokens WHERE is_nft = false
            "#,
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|token| token.id as u32)
        .unwrap_or(0);

        metrics::histogram!("sql.token.get_last_token_id", start.elapsed());
        Ok(TokenId(last_token_id))
    }

    pub async fn get_nft(&mut self, token_id: TokenId) -> QueryResult<Option<NFT>> {
        let start = Instant::now();
        let db_token = sqlx::query_as!(
            StorageNFT,
            r#"
                SELECT * FROM nft
                WHERE token_id = $1
                LIMIT 1
            "#,
            *token_id as i32
        )
        .fetch_optional(self.0.conn())
        .await?;
        metrics::histogram!("sql.token.get_nft", start.elapsed());
        Ok(db_token.map(|t| t.into()))
    }

    pub async fn get_nft_with_factories(
        &mut self,
        token_id: TokenId,
    ) -> QueryResult<Option<ApiNFT>> {
        let start = Instant::now();
        let db_token = sqlx::query_as!(
            StorageApiNFT,
            r#"
                SELECT nft.*, tokens.symbol, withdrawn_nfts_factories.factory_address as "withdrawn_factory?",
                    COALESCE(nft_factory.factory_address, server_config.nft_factory_addr) as "current_factory!"
                FROM nft
                INNER JOIN server_config
                    ON server_config.id = true
                INNER JOIN tokens
                    ON tokens.id = nft.token_id
                LEFT JOIN nft_factory
                    ON nft_factory.creator_id = nft.creator_account_id
                LEFT JOIN withdrawn_nfts_factories
                    ON withdrawn_nfts_factories.token_id = nft.token_id
                WHERE nft.token_id = $1
                LIMIT 1
            "#,
            *token_id as i32
        )
        .fetch_optional(self.0.conn())
        .await?;
        metrics::histogram!("sql.token.get_nft_with_factories", start.elapsed());
        Ok(db_token.map(|t| t.into()))
    }

    /// Stores nft factory addresses that are used for each of given nfts withdrawings
    pub async fn store_factories_for_withdrawn_nfts(
        &mut self,
        nfts: Vec<TokenId>,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        // 1. Get creator_account_id for each nft
        // 2. Get factory_address for each creator_account_id
        // 3. Match nfts with factory addresses
        // 4. Insert nfts and factory addresses

        let config = transaction.config_schema().load_config().await?;
        let default_factory_address = config
            .nft_factory_addr
            .ok_or_else(|| anyhow::anyhow!("Default nft factory is not stored in storage"))?;

        let nfts: Vec<i32> = nfts.into_iter().map(|id| *id as i32).collect();
        let nft_creators: Vec<StorageNFTCreator> = sqlx::query_as!(
            StorageNFTCreator,
            r#"
                SELECT token_id, creator_account_id FROM nft
                WHERE token_id = ANY($1)
            "#,
            &nfts
        )
        .fetch_all(transaction.conn())
        .await?;

        let creators_ids: Vec<i32> = nft_creators
            .iter()
            .map(|nft| nft.creator_account_id)
            .collect();
        let nft_factories: Vec<StorageNFTFactory> = sqlx::query_as!(
            StorageNFTFactory,
            r#"
                SELECT creator_id, factory_address FROM nft_factory
                WHERE creator_id = ANY($1)
            "#,
            &creators_ids
        )
        .fetch_all(transaction.conn())
        .await?;

        let mut address_by_creator = HashMap::new();
        for creator_with_factory in nft_factories {
            address_by_creator.insert(
                creator_with_factory.creator_id,
                creator_with_factory.factory_address,
            );
        }

        let mut token_ids = Vec::new();
        let mut factories = Vec::new();
        for nft_with_creator in nft_creators {
            token_ids.push(nft_with_creator.token_id);
            factories.push(
                address_by_creator
                    .get(&nft_with_creator.creator_account_id)
                    .unwrap_or(&default_factory_address)
                    .clone(),
            );
        }

        sqlx::query!(
            r#"
                INSERT INTO withdrawn_nfts_factories (token_id, factory_address)
                SELECT u.token_id, u.factory_address
                    FROM UNNEST ($1::int[], $2::text[])
                    AS u(token_id, factory_address)
            "#,
            &token_ids,
            &factories
        )
        .execute(transaction.conn())
        .await?;

        transaction.commit().await?;
        metrics::histogram!(
            "sql.token.store_factories_for_withdrawn_nfts",
            start.elapsed()
        );
        Ok(())
    }

    /// Given the numeric token ID, symbol or address, returns token.
    pub async fn get_token(&mut self, token_like: TokenLike) -> QueryResult<Option<Token>> {
        let start = Instant::now();
        let db_token = match token_like {
            TokenLike::Id(token_id) => {
                sqlx::query_as!(
                    DbToken,
                    r#"
                    SELECT * FROM tokens
                    WHERE id = $1
                    LIMIT 1
                    "#,
                    *token_id as i32
                )
                .fetch_optional(self.0.conn())
                .await?
            }
            TokenLike::Address(token_address) => {
                sqlx::query_as!(
                    DbToken,
                    r#"
                    SELECT * FROM tokens
                    WHERE address = $1
                    LIMIT 1
                    "#,
                    address_to_stored_string(&token_address)
                )
                .fetch_optional(self.0.conn())
                .await?
            }
            TokenLike::Symbol(token_symbol) => {
                sqlx::query_as!(
                    DbToken,
                    r#"
                    SELECT * FROM tokens
                    WHERE symbol = $1
                    LIMIT 1
                    "#,
                    token_symbol
                )
                .fetch_optional(self.0.conn())
                .await?
            }
        };

        metrics::histogram!("sql.token.get_token", start.elapsed());
        Ok(db_token.map(|t| t.into()))
    }

    pub async fn get_token_market_volume(
        &mut self,
        token_id: TokenId,
    ) -> QueryResult<Option<TokenMarketVolume>> {
        let start = Instant::now();
        let db_market_volume = sqlx::query_as!(
            DBMarketVolume,
            r#"
            SELECT * FROM ticker_market_volume
            WHERE token_id = $1
            LIMIT 1
            "#,
            *token_id as i32
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.get_market_volume", start.elapsed());
        Ok(db_market_volume.map(|p| p.into()))
    }

    pub async fn update_token_market_volume(
        &mut self,
        token_id: TokenId,
        market_volume: TokenMarketVolume,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let market_volume_rounded =
            ratio_to_big_decimal(&market_volume.market_volume, STORED_USD_PRICE_PRECISION);
        sqlx::query!(
            r#"
            INSERT INTO ticker_market_volume ( token_id, market_volume, last_updated )
            VALUES ( $1, $2, $3 )
            ON CONFLICT (token_id)
            DO
              UPDATE SET market_volume = $2, last_updated = $3
            "#,
            *token_id as i32,
            market_volume_rounded.clone(),
            market_volume.last_updated
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.update_market_volume", start.elapsed());
        Ok(())
    }

    /// Given token id, returns its price in USD and a timestamp of the last update.
    pub async fn get_historical_ticker_price(
        &mut self,
        token_id: TokenId,
    ) -> QueryResult<Option<TokenPrice>> {
        let start = Instant::now();
        let db_price = sqlx::query_as!(
            DbTickerPrice,
            r#"
            SELECT * FROM ticker_price
            WHERE token_id = $1
            LIMIT 1
            "#,
            *token_id as i32
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.get_historical_ticker_price", start.elapsed());
        Ok(db_price.map(|p| p.into()))
    }

    /// Updates price in USD for the given token.
    ///
    /// Note, that the price precision cannot be greater than `STORED_USD_PRICE_PRECISION`,
    /// so the number might get rounded.
    pub async fn update_historical_ticker_price(
        &mut self,
        token_id: TokenId,
        price: TokenPrice,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let usd_price_rounded = ratio_to_big_decimal(&price.usd_price, STORED_USD_PRICE_PRECISION);
        sqlx::query!(
            r#"
            INSERT INTO ticker_price ( token_id, usd_price, last_updated )
            VALUES ( $1, $2, $3 )
            ON CONFLICT (token_id)
            DO
              UPDATE SET usd_price = $2, last_updated = $3
            "#,
            *token_id as i32,
            usd_price_rounded.clone(),
            price.last_updated
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.update_historical_ticker_price", start.elapsed());
        Ok(())
    }

    pub async fn store_nft_factory(
        &mut self,
        creator_id: AccountId,
        creator_address: Address,
        factory_address: Address,
    ) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            r#"
            INSERT INTO nft_factory ( creator_id, factory_address, creator_address )
            VALUES ( $1, $2, $3 )
            ON CONFLICT ( creator_id ) 
            DO UPDATE 
            SET factory_address = $2 
            "#,
            creator_id.0 as i32,
            address_to_stored_string(&factory_address),
            address_to_stored_string(&creator_address),
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.store_nft_factory", start.elapsed());
        Ok(())
    }
}
