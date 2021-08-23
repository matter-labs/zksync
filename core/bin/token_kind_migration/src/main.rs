use zksync_config::ZkSyncConfig;
use zksync_contracts::erc20_contract;
use zksync_eth_client::EthereumGateway;
use zksync_storage::StorageProcessor;
use zksync_types::{TokenKind, U256};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ZkSyncConfig::from_env();
    let eth_gateway = EthereumGateway::from_config(&config);

    let mut storage = StorageProcessor::establish_connection().await?;
    let mut transaction = storage.start_transaction().await?;

    let tokens = transaction.tokens_schema().load_tokens().await?;
    let mut non_erc20_tokens = Vec::new();
    for (_, mut token) in tokens {
        if token.address == Default::default() {
            continue;
        }
        let is_erc20 = eth_gateway
            .call_contract_function::<U256, _, _, _>(
                "balanceOf",
                token.address,
                None,
                Default::default(),
                None,
                token.address,
                erc20_contract(),
            )
            .await
            .is_ok();
        if !is_erc20 {
            non_erc20_tokens.push(token.symbol.clone());
            token.kind = TokenKind::None;
            transaction
                .tokens_schema()
                .store_or_update_token(token)
                .await?;
        }
    }

    transaction.commit().await?;

    println!("Token kinds are successfully updated");
    println!("List of non-ERC20 tokens: {:?}", non_erc20_tokens);
    Ok(())
}
