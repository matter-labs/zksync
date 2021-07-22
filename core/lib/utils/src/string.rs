/// Strip the common prefixes off the HEX-encoded string
pub fn remove_prefix(hex: &str) -> &str {
    if let Some(hex) = hex.strip_prefix("0x") {
        hex
    } else if let Some(hex) = hex.strip_prefix("sync-bl:") {
        hex
    } else if let Some(hex) = hex.strip_prefix("sync-tx:") {
        hex
    } else {
        hex
    }
}
