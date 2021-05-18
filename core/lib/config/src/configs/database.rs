// Built-in uses
use std::time;

// External uses
use serde::Deserialize;

// Local uses
use crate::envy_load;

/// Used database configuration.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct DBConfig {
    /// Amount of open connections to the database held by server in the pool.
    pub pool_size: usize,
    /// Database URL.
    pub url: String,
    /// Rejected transactions will be stored in the database for this amount of hours.
    pub rejected_transactions_max_age: u64,
    /// Sleep time (in hours) of the actor responsible for deleting failed transactions from the database.
    pub rejected_transactions_cleaner_interval: u64,
}

impl DBConfig {
    const SECS_PER_HOUR: u64 = 3600;

    pub fn from_env() -> Self {
        envy_load!("database", "DATABASE_")
    }

    pub fn rejected_transactions_max_age(&self) -> chrono::Duration {
        chrono::Duration::hours(self.rejected_transactions_max_age as i64)
    }

    pub fn rejected_transactions_cleaner_interval(&self) -> time::Duration {
        time::Duration::from_secs(self.rejected_transactions_cleaner_interval * Self::SECS_PER_HOUR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::set_env;

    fn expected_config() -> DBConfig {
        DBConfig {
            pool_size: 10,
            url: "postgres://postgres@localhost/plasma".into(),
            rejected_transactions_max_age: 336,
            rejected_transactions_cleaner_interval: 24,
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
DATABASE_POOL_SIZE="10"
DATABASE_URL="postgres://postgres@localhost/plasma"
DATABASE_REJECTED_TRANSACTIONS_MAX_AGE="336"
DATABASE_REJECTED_TRANSACTIONS_CLEANER_INTERVAL="24"
        "#;
        set_env(config);

        let actual = DBConfig::from_env();
        assert_eq!(actual, expected_config());
    }
}
