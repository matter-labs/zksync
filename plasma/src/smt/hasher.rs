// Hasher trait

pub trait IntoBits {
    fn into_bits(&self) -> Vec<bool>;
}

pub trait Hasher<Hash> {
    fn hash_bits<I: IntoIterator<Item=bool>>(&self, value: I) -> Hash;
    fn compress(&self, lhs: &Hash, rhs: &Hash, i: usize) -> Hash;
    fn empty_hash(&self) -> Hash;
}
