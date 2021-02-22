use reqwest::{Client, Url};

pub struct MatterMostNotifier {
    username: String,
    webhook_url: Url,
    client: Client,
}

impl MatterMostNotifier {
    pub fn new(username: String, webhook_url: Url) -> Self {
        Self {
            username,
            webhook_url,
            client: Client::new(),
        }
    }

    pub async fn send_notify(&self, text: &str) -> anyhow::Result<()> {
        let parameters = serde_json::json!({
            "username": serde_json::to_value(&self.username)?,
            "text": serde_json::to_value(text)?,
        });

        self.client
            .post(self.webhook_url.clone())
            .json(&parameters)
            .send()
            .await?;

        Ok(())
    }
}
