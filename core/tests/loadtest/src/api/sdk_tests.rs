// Built-in uses
// External uses
// Workspace uses
use zksync_types::TxFeeTypes;
// Local uses
use crate::monitor::Monitor;

use super::ApiTestsBuilder;

pub fn wire_tests<'a>(builder: ApiTestsBuilder<'a>, monitor: &'a Monitor) -> ApiTestsBuilder<'a> {
    builder
        .append("provider/tokens", move || async move {
            monitor.provider.tokens().await?;
            Ok(())
        })
        .append("provider/contract_address", move || async move {
            monitor.provider.contract_address().await?;
            Ok(())
        })
        .append("provider/account_info", move || async move {
            monitor
                .provider
                .account_info(monitor.api_data_pool.read().await.random_address().0)
                .await?;
            Ok(())
        })
        .append("provider/get_tx_fee", move || async move {
            monitor
                .provider
                .get_tx_fee(
                    TxFeeTypes::FastWithdraw,
                    monitor.api_data_pool.read().await.random_address().0,
                    "ETH",
                )
                .await?;
            Ok(())
        })
        .append("provider/tx_info", move || async move {
            monitor
                .provider
                .tx_info(monitor.api_data_pool.read().await.random_tx_hash())
                .await?;
            Ok(())
        })
        .append("provider/ethop_info", move || async move {
            monitor
                .provider
                .ethop_info(
                    monitor
                        .api_data_pool
                        .read()
                        .await
                        .random_priority_op()
                        .serial_id as u32,
                )
                .await?;
            Ok(())
        })
}
