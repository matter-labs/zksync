use models::plasma::Fr;

/// Serialize `Fr`.
pub fn fr_to_bytes(fr: &Fr) -> Vec<u8> {
    use ff::{PrimeField, PrimeFieldRepr};
    let mut buff: Vec<u8> = Vec::new();
    fr.into_repr().write_be(&mut buff).unwrap();
    buff
}

/// Deserialize `Fr` from trusted source.
pub fn fr_from_bytes(bytes: Vec<u8>) -> Fr {
    use ff::{PrimeField, PrimeFieldRepr};
    let mut fr_repr = <Fr as PrimeField>::Repr::default();
    fr_repr.read_be(&*bytes).unwrap();
    Fr::from_repr(fr_repr).unwrap()
}
