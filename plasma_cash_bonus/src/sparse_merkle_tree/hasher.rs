// Hasher trait

pub trait Hasher<Hash> {
    fn hash_bits<I: IntoIterator<Item=bool>>(&self, value: I) -> Hash;
    fn compress(&self, lhs: &Hash, rhs: &Hash, i: usize) -> Hash;
}
