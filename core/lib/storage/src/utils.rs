use zksync_types::Address;

pub fn address_to_stored_string(address: &Address) -> String {
    format!("0x{:x}", address)
}

pub fn stored_str_address_to_address(address: &str) -> Address {
    assert_eq!(address.len(), 42, "db stored token address length");
    address[2..]
        .parse()
        .expect("failed to parse stored db address")
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn address_store_roundtrip() {
        let address = Address::random();
        let stored_address = address_to_stored_string(&address);
        assert_eq!(address, stored_str_address_to_address(&stored_address));
    }
}
