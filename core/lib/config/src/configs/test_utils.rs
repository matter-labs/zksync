// Built-in uses.
use std::{env, str::FromStr};
// Workspace uses
use zksync_types::{Address, H256};

/// Parses the provided fixture in a form of `VARIABLE_NAME=variable_value` lines and
/// sets the corresponding environment variables.
pub fn set_env(fixture: &str) {
    for line in fixture.split('\n').map(str::trim) {
        if line.is_empty() {
            // Skip empty lines.
            continue;
        }

        let elements: Vec<_> = line.split('=').collect();
        assert_eq!(
            elements.len(),
            2,
            "Incorrect line for setting environment variable: {}",
            line
        );

        let variable_name = elements[0];
        let variable_value = elements[1].trim_matches('"');

        env::set_var(variable_name, variable_value);
    }
}

/// Parses the address panicking upon deserialization failure.
pub fn addr(addr_str: &str) -> Address {
    Address::from_str(addr_str).expect("Incorrect address string")
}

/// Parses the H256 panicking upon deserialization failure.
pub fn hash(addr_str: &str) -> H256 {
    H256::from_str(addr_str).expect("Incorrect hash string")
}
