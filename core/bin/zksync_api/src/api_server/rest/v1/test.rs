//! API testing helpers.

// Built-in uses

// External uses
use actix_web::{web, App, Scope};

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_storage::ConnectionPool;

// Local uses
use super::client::Client;

#[derive(Debug, Clone)]
pub struct TestServerConfig {
    pub env_options: ConfigurationOptions,
    pub pool: ConnectionPool,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            env_options: ConfigurationOptions::from_env(),
            pool: ConnectionPool::new(Some(1)),
        }
    }
}

impl TestServerConfig {
    pub fn start_server<F>(&self, scope_factory: F) -> (Client, actix_web::test::TestServer)
    where
        F: Fn(&TestServerConfig) -> Scope + Clone + Send + 'static,
    {
        let this = self.clone();
        let server = actix_web::test::start(move || {
            App::new().service(web::scope("/api/v1").service(scope_factory(&this)))
        });

        let mut url = server.url("");
        url.pop(); // Pop last '/' symbol.

        let client = Client::new(url);
        (client, server)
    }
}
