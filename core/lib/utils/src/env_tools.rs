use std::{env, str::FromStr};

/// Obtains the environment variable value.
/// Panics if there is no environment variable with provided name set.
pub fn get_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|e| panic!("Env var {} missing, {}", name, e))
}

/// Obtains the environment variable value and parses it using the `FromStr` type implementation.
/// Panics if there is no environment variable with provided name set, or the value cannot be parsed.
pub fn parse_env<F>(name: &str) -> F
where
    F: FromStr,
    F::Err: std::fmt::Debug,
{
    get_env(name)
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse environment variable {}: {:?}", name, e))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_env_tools() {
        const KEY: &str = "KEY";
        // Our test environment variable.
        env::set_var(KEY, "123");
        assert_eq!(get_env(KEY), "123");
        assert_eq!(parse_env::<i32>(KEY), 123);
    }
}
