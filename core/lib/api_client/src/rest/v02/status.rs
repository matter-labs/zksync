use crate::rest::client::{Client, Result};
use zksync_api_types::v02::Response;

impl Client {
    pub async fn status(&self) -> Result<Response> {
        self.get_with_scope(super::API_V02_SCOPE, "networkStatus")
            .send()
            .await
    }
}
