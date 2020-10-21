// Built-in uses
// External uses
// Workspace uses
use zksync_utils::parse_env;
// Local uses
use super::{ApiDataPool, ApiTestsBuilder};
use crate::monitor::Monitor;

#[derive(Debug, Clone)]
struct RestApiClient {
    inner: reqwest::Client,
    url: String,
    pool: ApiDataPool,
}

impl RestApiClient {
    pub fn new(url: String, pool: ApiDataPool) -> Self {
        Self {
            inner: reqwest::Client::new(),
            url,
            pool,
        }
    }

    fn api_prefix(&self) -> String {
        [&self.url, "/api/v0.1"].concat()
    }

    async fn get(&self, method: impl AsRef<str>) -> reqwest::Result<serde_json::Value> {
        let url = [&self.api_prefix(), "/", method.as_ref()].concat();
        self.inner.get(&url).send().await?.json().await
    }

    pub async fn testnet_config(&self) -> anyhow::Result<()> {
        self.get("testnet_config").await?;
        Ok(())
    }

    pub async fn status(&self) -> anyhow::Result<()> {
        self.get("status").await?;
        Ok(())
    }

    pub async fn tokens(&self) -> anyhow::Result<()> {
        self.get("tokens").await?;
        Ok(())
    }
}

macro_rules! declare_tests {
    (($builder:expr, $client:expr) => $($method:ident,)*) => {
        $builder $(
            .append(concat!("rest_api/", stringify!($method)), {
                let client = $client.clone();
                move || {
                    let client = client.clone();
                    async move {
                        client.$method().await
                    }
                }
            })
        )* ;
    }
}

pub fn wire_tests<'a>(builder: ApiTestsBuilder<'a>, monitor: &'a Monitor) -> ApiTestsBuilder<'a> {
    // TODO add this field to the ConfigurationOptions.
    let rest_api_url = parse_env::<String>("REST_API_ADDR");

    let client = RestApiClient::new(rest_api_url, monitor.api_data_pool.clone());
    declare_tests!(
        (builder, client) =>
            testnet_config,
            status,
            tokens,
    )
}
