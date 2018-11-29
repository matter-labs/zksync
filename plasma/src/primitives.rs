use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};

// TODO: replace Vec with Iterator?

pub trait GetBits {
    fn get_bits_le(&self) -> Vec<bool>;
}

pub trait GetBitsFixed {

    /// Get exactly `n` bits from the value in little endian order
    /// If `n` is larger than value bit length, it is padded with `false`
    /// for the result to exactly match `n`
    fn get_bits_le_fixed(&self, n: usize) -> Vec<bool>;
}

impl<Fr: PrimeField> GetBitsFixed for Fr {

    fn get_bits_le_fixed(&self, n: usize) -> Vec<bool> {
        let mut r: Vec<bool> = Vec::with_capacity(n);
        r.extend(BitIterator::new(self.into_repr()));
        r.reverse();
        r.truncate(n);
        let len = r.len();
        r.extend((len..n).map(|_| false));
        r
    }
}

#[test]
fn test_get_bits() {
    use pairing::bn256::{Fr};

    // 12 = b1100, 3 lowest bits in little endian encoding are: 0, 0, 1.
    let bits = Fr::from_str("12").unwrap().get_bits_le_fixed(3);
    assert_eq!(bits, vec![false, false, true]);

    let bits = Fr::from_str("0").unwrap().get_bits_le_fixed(512);
    assert_eq!(bits, vec!(false; 512));
}