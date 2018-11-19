use super::{CurveProjective, PrimeField, PrimeFieldRepr};

/// Replaces the contents of `table` with a w-NAF window table for the given window size.
pub(crate) fn wnaf_table<G: CurveProjective>(table: &mut Vec<G>, mut base: G, window: usize) {
    table.truncate(0);
    table.reserve(1 << (window - 1));

    let mut dbl = base;
    dbl.double();

    for _ in 0..(1 << (window - 1)) {
        table.push(base);
        base.add_assign(&dbl);
    }
}

/// Replaces the contents of `wnaf` with the w-NAF representation of a scalar.
pub(crate) fn wnaf_form<S: PrimeFieldRepr>(wnaf: &mut Vec<i64>, mut c: S, window: usize) {
    wnaf.truncate(0);

    while !c.is_zero() {
        let mut u;
        if c.is_odd() {
            u = (c.as_ref()[0] % (1 << (window + 1))) as i64;

            if u > (1 << window) {
                u -= 1 << (window + 1);
            }

            if u > 0 {
                c.sub_noborrow(&S::from(u as u64));
            } else {
                c.add_nocarry(&S::from((-u) as u64));
            }
        } else {
            u = 0;
        }

        wnaf.push(u);

        c.div2();
    }
}

/// Performs w-NAF exponentiation with the provided window table and w-NAF form scalar.
///
/// This function must be provided a `table` and `wnaf` that were constructed with
/// the same window size; otherwise, it may panic or produce invalid results.
pub(crate) fn wnaf_exp<G: CurveProjective>(table: &[G], wnaf: &[i64]) -> G {
    let mut result = G::zero();

    let mut found_one = false;

    for n in wnaf.iter().rev() {
        if found_one {
            result.double();
        }

        if *n != 0 {
            found_one = true;

            if *n > 0 {
                result.add_assign(&table[(n / 2) as usize]);
            } else {
                result.sub_assign(&table[((-n) / 2) as usize]);
            }
        }
    }

    result
}

/// A "w-ary non-adjacent form" exponentiation context.
#[derive(Debug)]
pub struct Wnaf<W, B, S> {
    base: B,
    scalar: S,
    window_size: W,
}

impl<G: CurveProjective> Wnaf<(), Vec<G>, Vec<i64>> {
    /// Construct a new wNAF context without allocating.
    pub fn new() -> Self {
        Wnaf {
            base: vec![],
            scalar: vec![],
            window_size: (),
        }
    }

    /// Given a base and a number of scalars, compute a window table and return a `Wnaf` object that
    /// can perform exponentiations with `.scalar(..)`.
    pub fn base(&mut self, base: G, num_scalars: usize) -> Wnaf<usize, &[G], &mut Vec<i64>> {
        // Compute the appropriate window size based on the number of scalars.
        let window_size = G::recommended_wnaf_for_num_scalars(num_scalars);

        // Compute a wNAF table for the provided base and window size.
        wnaf_table(&mut self.base, base, window_size);

        // Return a Wnaf object that immutably borrows the computed base storage location,
        // but mutably borrows the scalar storage location.
        Wnaf {
            base: &self.base[..],
            scalar: &mut self.scalar,
            window_size,
        }
    }

    /// Given a scalar, compute its wNAF representation and return a `Wnaf` object that can perform
    /// exponentiations with `.base(..)`.
    pub fn scalar(
        &mut self,
        scalar: <<G as CurveProjective>::Scalar as PrimeField>::Repr,
    ) -> Wnaf<usize, &mut Vec<G>, &[i64]> {
        // Compute the appropriate window size for the scalar.
        let window_size = G::recommended_wnaf_for_scalar(scalar);

        // Compute the wNAF form of the scalar.
        wnaf_form(&mut self.scalar, scalar, window_size);

        // Return a Wnaf object that mutably borrows the base storage location, but
        // immutably borrows the computed wNAF form scalar location.
        Wnaf {
            base: &mut self.base,
            scalar: &self.scalar[..],
            window_size,
        }
    }
}

impl<'a, G: CurveProjective> Wnaf<usize, &'a [G], &'a mut Vec<i64>> {
    /// Constructs new space for the scalar representation while borrowing
    /// the computed window table, for sending the window table across threads.
    pub fn shared(&self) -> Wnaf<usize, &'a [G], Vec<i64>> {
        Wnaf {
            base: self.base,
            scalar: vec![],
            window_size: self.window_size,
        }
    }
}

impl<'a, G: CurveProjective> Wnaf<usize, &'a mut Vec<G>, &'a [i64]> {
    /// Constructs new space for the window table while borrowing
    /// the computed scalar representation, for sending the scalar representation
    /// across threads.
    pub fn shared(&self) -> Wnaf<usize, Vec<G>, &'a [i64]> {
        Wnaf {
            base: vec![],
            scalar: self.scalar,
            window_size: self.window_size,
        }
    }
}

impl<B, S: AsRef<[i64]>> Wnaf<usize, B, S> {
    /// Performs exponentiation given a base.
    pub fn base<G: CurveProjective>(&mut self, base: G) -> G
    where
        B: AsMut<Vec<G>>,
    {
        wnaf_table(self.base.as_mut(), base, self.window_size);
        wnaf_exp(self.base.as_mut(), self.scalar.as_ref())
    }
}

impl<B, S: AsMut<Vec<i64>>> Wnaf<usize, B, S> {
    /// Performs exponentiation given a scalar.
    pub fn scalar<G: CurveProjective>(
        &mut self,
        scalar: <<G as CurveProjective>::Scalar as PrimeField>::Repr,
    ) -> G
    where
        B: AsRef<[G]>,
    {
        wnaf_form(self.scalar.as_mut(), scalar, self.window_size);
        wnaf_exp(self.base.as_ref(), self.scalar.as_mut())
    }
}
