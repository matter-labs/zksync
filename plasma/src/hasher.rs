// Hasher trait

pub trait Factory {
    fn new() -> Self;
}

pub trait Hasher<T, Hash> {
    fn hash(&self, value: &T) -> Hash;
    fn compress(&self, lhs: &Hash, rhs: &Hash, i: usize) -> Hash;
    fn empty_hash(&self) -> Hash;
}
