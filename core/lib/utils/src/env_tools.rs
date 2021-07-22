use std::{env, iter::FromIterator, str::FromStr};

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

/// Similar to `parse_env`, but also takes a function to change the variable value before parsing.
pub fn parse_env_with<T, F>(name: &str, f: F) -> T
where
    T: FromStr,
    T::Err: std::fmt::Debug,
    F: FnOnce(&str) -> &str,
{
    let env_var = get_env(name);

    f(&env_var)
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse environment variable {}: {:?}", name, e))
}

/// Obtains the environment variable value and on success parses it using the `FromStr` type implementation.
/// Panics if value cannot be parsed.
pub fn parse_env_if_exists<F>(name: &str) -> Option<F>
where
    F: FromStr,
    F::Err: std::fmt::Debug,
{
    env::var(name)
        .map(|var| {
            var.parse().unwrap_or_else(|e| {
                panic!("Failed to parse environment variable {}: {:?}", name, e)
            })
        })
        .ok()
}

/// Obtains the environment comma separated variables into collection.
pub fn parse_env_to_collection<F, I>(name: &str) -> F
where
    I: FromStr,
    I::Err: std::fmt::Debug,
    F: FromIterator<I>,
{
    get_env(name)
        .split(',')
        .map(|p| p.parse::<I>().unwrap())
        .collect()
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
        assert_eq!(parse_env_if_exists::<i32>(KEY), Some(123));

        env::remove_var(KEY);
        assert_eq!(parse_env_if_exists::<i32>(KEY), None);

        env::set_var(KEY, "ABC123");
        let parsed: i32 = parse_env_with(KEY, |key| &key[3..]);
        assert_eq!(parsed, 123);
    }
}
