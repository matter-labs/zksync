// Built-in uses
use std::net::SocketAddr;

// External uses
use serde::Deserialize;

// Local uses
use crate::envy_load;

/// Configuration for the Event listener crate.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct EventListenerConfig {
    /// The port used by the server.
    pub ws_port: u16,
    /// URL for access to the EventListener server.
    pub ws_url: String,
    /// PostgreSQL channel name to listen on. Must be equal to the one
    /// hardcoded into database migrations.
    pub channel_name: String,
}

impl EventListenerConfig {
    pub fn from_env() -> Self {
        envy_load!("event_listener", "EVENT_LISTENER_")
    }

    pub fn ws_bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.ws_port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::set_env;
    use std::net::IpAddr;

    fn expected_config() -> EventListenerConfig {
        EventListenerConfig {
            ws_port: 65535,
            ws_url: "ws://localhost:12345".into(),
            channel_name: "zksync_event_channel".into(),
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
EVENT_LISTENER_WS_URL="ws://localhost:12345"
EVENT_LISTENER_WS_PORT="65535"
EVENT_LISTENER_CHANNEL_NAME="zksync_event_channel"
        "#;
        set_env(config);

        let actual = EventListenerConfig::from_env();
        assert_eq!(actual, expected_config());
    }

    #[test]
    fn test_bind_addr() {
        let config = expected_config();
        let bind_addr: IpAddr = "0.0.0.0".parse().unwrap();

        assert_eq!(
            config.ws_bind_addr(),
            SocketAddr::new(bind_addr, config.ws_port)
        );
    }
}
