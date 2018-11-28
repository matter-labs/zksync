// Hasher trait

pub trait Factory {
    fn new() -> Self;
}

pub trait IntoBits<I: IntoIterator<Item=bool>> {
    fn into_bits(&self) -> I;
}

pub trait Hasher<T, Hash> {

    fn hash(&self, value: &T) -> Hash;
    //fn hash_bits<I: IntoIterator<Item=bool>>(&self, value: I) -> Hash;
    fn compress(&self, lhs: &Hash, rhs: &Hash, i: usize) -> Hash;
    fn empty_hash(&self) -> Hash;
}
