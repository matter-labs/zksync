use crate::rest::client::{Client, Result};
use zksync_api_types::v02::Response;

impl Client {
    pub async fn config(&self) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, "config")
            .send()
            .await
    }
}
