use ff::{Field, PrimeField, PrimeFieldRepr};

// TODO: replace Vec with Iterator?

pub trait GetBits {
    fn get_bits_le(&self) -> Vec<bool>;
}

pub trait GetBitsFixed {
    fn get_bits_le_fixed(&self, n: usize) -> Vec<bool>;
}

impl<Fr: PrimeField> GetBitsFixed for Fr {

    fn get_bits_le_fixed(&self, n: usize) -> Vec<bool> {
        let mut acc = Vec::with_capacity(n);
        let mut t = self.into_repr().clone();
        for i in 0..n {
            acc.push(t.is_odd());
            t.shr(1);
        }
        acc
    }
}

#[test]
fn test_get_bits() {
    use pairing::bn256::{Fr};

    // 12 = b1100, 3 lowest bits in little endian encoding are: 0, 0, 1.
    let bits = Fr::from_str("12").unwrap().get_bits_le_fixed(3);
    assert_eq!(bits, vec![false, false, true]);
}