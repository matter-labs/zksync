// Built-in deps
use std::collections::VecDeque;
use std::string::ToString;
// External deps
// Workspace deps

/// Formats amount in wei to tokens.
/// Behaves just like js ethers.utils.formatEther
pub fn format_ether(wei: &impl ToString) -> String {
    const N_DECIMAL: usize = 18;
    let mut chars = wei.to_string().drain(..).collect::<VecDeque<char>>();
    while chars.len() < N_DECIMAL {
        chars.push_front('0');
    }
    chars.insert(chars.len() - N_DECIMAL, '.');
    if *chars.front().unwrap() == '.' {
        chars.push_front('0');
    }
    while *chars.back().unwrap() == '0' {
        chars.pop_back();
    }
    if *chars.back().unwrap() == '.' {
        chars.push_back('0');
    }
    chars.iter().collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_format_ether() {
        let vals = [
            ("0", "0.0"),
            ("110", "0.00000000000000011"),
            ("11000000", "0.000000000011"),
            ("10001000000", "0.000000010001"),
            ("10010000000", "0.00000001001"),
            ("10000000000000000001", "10.000000000000000001"),
            ("11000000000000000000", "11.0"),
            ("100000010000000000000", "100.00001"),
            ("1000000000000000100000", "1000.0000000000001"),
            ("10100000000000000000000", "10100.0"),
            ("20000000000000000000000", "20000.0"),
        ];
        for (input, output) in &vals {
            assert_eq!(format_ether(input), output.to_owned());
        }
    }
}
