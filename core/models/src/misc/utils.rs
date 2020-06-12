// Built-in deps
use std::collections::VecDeque;
use std::string::ToString;
// External deps
// Workspace deps

/// Formats amount in wei to tokens with precision.
/// Behaves just like ethers.utils.formatUnits
pub fn format_units(wei: &impl ToString, units: u8) -> String {
    let mut chars = wei.to_string().drain(..).collect::<VecDeque<char>>();
    while chars.len() < units as usize {
        chars.push_front('0');
    }
    chars.insert(chars.len() - units as usize, '.');
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

/// Formats amount in wei to tokens.
/// Behaves just like js ethers.utils.formatEther
pub fn format_ether(wei: &impl ToString) -> String {
    format_units(wei, 18)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_format_units() {
        // Test vector of (decimals, wei input, expected output)
        let vals = vec![
            (0, "1000000000000000100000", "1000000000000000100000.0"),
            (1, "0", "0.0"),
            (1, "11000000000000000000", "1100000000000000000.0"),
            (2, "0", "0.0"),
            (2, "1000000000000000100000", "10000000000000001000.0"),
            (4, "10001000000", "1000100.0"),
            (4, "10100000000000000000000", "1010000000000000000.0"),
            (4, "110", "0.011"),
            (6, "1000000000000000100000", "1000000000000000.1"),
            (8, "0", "0.0"),
            (8, "10100000000000000000000", "101000000000000.0"),
            (8, "110", "0.0000011"),
            (9, "10000000000000000001", "10000000000.000000001"),
            (9, "11000000", "0.011"),
            (9, "11000000000000000000", "11000000000.0"),
            (10, "10001000000", "1.0001"),
            (10, "20000000000000000000000", "2000000000000.0"),
            (11, "0", "0.0"),
            (11, "10100000000000000000000", "101000000000.0"),
            (12, "1000000000000000100000", "1000000000.0000001"),
            (12, "10001000000", "0.010001"),
            (12, "10010000000", "0.01001"),
            (12, "110", "0.00000000011"),
            (13, "10010000000", "0.001001"),
            (14, "10010000000", "0.0001001"),
            (14, "110", "0.0000000000011"),
            (15, "0", "0.0"),
            (17, "1000000000000000100000", "10000.000000000001"),
            (17, "10001000000", "0.00000010001"),
            (18, "1000000000000000100000", "1000.0000000000001"),
        ];

        for (dec, input, output) in vals {
            assert_eq!(format_units(&input, dec), output);
        }
    }
}
