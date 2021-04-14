/// Trait that allows accessing all the possible variants of a sequence.
pub trait All: Sized {
    fn all() -> &'static [Self];
}

/// Trait that extends `All` trait with the corresponding expected probability.
pub trait AllWeighted: Sized {
    fn all_weighted() -> &'static [(Self, f32)];
}
