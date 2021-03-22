use matter_most_notifier::MatterMostNotifier;
use reqwest::Url;
use zksync_types::tokens::Token;

mod matter_most_notifier;

pub struct Notifier {
    matter_most_notifier: MatterMostNotifier,
}

impl Notifier {
    pub fn with_mattermost(webhook_url: Url) -> Self {
        Self {
            matter_most_notifier: MatterMostNotifier::new(webhook_url),
        }
    }

    pub async fn send_new_token_notify(&self, token: Token) -> anyhow::Result<()> {
        let token_info_msg = format!(
            "New token: id = {}, address = {}, symbol = {}, decimals = {}",
            token.id, token.address, token.symbol, token.decimals,
        );
        self.matter_most_notifier
            .send_notify("token_handler_bot", &token_info_msg)
            .await?;

        Ok(())
    }
}
