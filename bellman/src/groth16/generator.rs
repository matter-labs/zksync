extern crate time;

use super::super::verbose_flag;

use self::time::PreciseTime;

use rand::Rng;

use std::sync::Arc;

use pairing::{
    Engine,
    Wnaf,
    CurveProjective,
    CurveAffine
};

use ff::{    
    PrimeField,
    Field
};

use super::{
    Parameters,
    VerifyingKey
};

use ::{
    SynthesisError,
    Circuit,
    ConstraintSystem,
    LinearCombination,
    Variable,
    Index
};

use ::domain::{
    EvaluationDomain,
    Scalar
};

use ::multicore::{
    Worker
};

/// Generates a random common reference string for
/// a circuit.
pub fn generate_random_parameters<E, C, R>(
    circuit: C,
    rng: &mut R
) -> Result<Parameters<E>, SynthesisError>
    where E: Engine, C: Circuit<E>, R: Rng
{
    let g1 = rng.gen();
    let g2 = rng.gen();
    let alpha = rng.gen();
    let beta = rng.gen();
    let gamma = rng.gen();
    let delta = rng.gen();
    let tau = rng.gen();

    generate_parameters::<E, C>(
        circuit,
        g1,
        g2,
        alpha,
        beta,
        gamma,
        delta,
        tau
    )
}

/// This is our assembly structure that we'll use to synthesize the
/// circuit into a QAP.
struct KeypairAssembly<E: Engine> {
    num_inputs: usize,
    num_aux: usize,
    num_constraints: usize,
    at_inputs: Vec<Vec<(E::Fr, usize)>>,
    bt_inputs: Vec<Vec<(E::Fr, usize)>>,
    ct_inputs: Vec<Vec<(E::Fr, usize)>>,
    at_aux: Vec<Vec<(E::Fr, usize)>>,
    bt_aux: Vec<Vec<(E::Fr, usize)>>,
    ct_aux: Vec<Vec<(E::Fr, usize)>>
}

impl<E: Engine> ConstraintSystem<E> for KeypairAssembly<E> {
    type Root = Self;

    fn alloc<F, A, AR>(
        &mut self,
        _: A,
        _: F
    ) -> Result<Variable, SynthesisError>
        where F: FnOnce() -> Result<E::Fr, SynthesisError>, A: FnOnce() -> AR, AR: Into<String>
    {
        // There is no assignment, so we don't even invoke the
        // function for obtaining one.

        let index = self.num_aux;
        self.num_aux += 1;

        self.at_aux.push(vec![]);
        self.bt_aux.push(vec![]);
        self.ct_aux.push(vec![]);

        Ok(Variable(Index::Aux(index)))
    }

    fn alloc_input<F, A, AR>(
        &mut self,
        _: A,
        _: F
    ) -> Result<Variable, SynthesisError>
        where F: FnOnce() -> Result<E::Fr, SynthesisError>, A: FnOnce() -> AR, AR: Into<String>
    {
        // There is no assignment, so we don't even invoke the
        // function for obtaining one.

        let index = self.num_inputs;
        self.num_inputs += 1;

        self.at_inputs.push(vec![]);
        self.bt_inputs.push(vec![]);
        self.ct_inputs.push(vec![]);

        Ok(Variable(Index::Input(index)))
    }

    fn enforce<A, AR, LA, LB, LC>(
        &mut self,
        _: A,
        a: LA,
        b: LB,
        c: LC
    )
        where A: FnOnce() -> AR, AR: Into<String>,
              LA: FnOnce(LinearCombination<E>) -> LinearCombination<E>,
              LB: FnOnce(LinearCombination<E>) -> LinearCombination<E>,
              LC: FnOnce(LinearCombination<E>) -> LinearCombination<E>
    {
        fn eval<E: Engine>(
            l: LinearCombination<E>,
            inputs: &mut [Vec<(E::Fr, usize)>],
            aux: &mut [Vec<(E::Fr, usize)>],
            this_constraint: usize
        )
        {
            for (index, coeff) in l.0 {
                match index {
                    Variable(Index::Input(id)) => inputs[id].push((coeff, this_constraint)),
                    Variable(Index::Aux(id)) => aux[id].push((coeff, this_constraint))
                }
            }
        }

        eval(a(LinearCombination::zero()), &mut self.at_inputs, &mut self.at_aux, self.num_constraints);
        eval(b(LinearCombination::zero()), &mut self.bt_inputs, &mut self.bt_aux, self.num_constraints);
        eval(c(LinearCombination::zero()), &mut self.ct_inputs, &mut self.ct_aux, self.num_constraints);

        self.num_constraints += 1;
    }

    fn push_namespace<NR, N>(&mut self, _: N)
        where NR: Into<String>, N: FnOnce() -> NR
    {
        // Do nothing; we don't care about namespaces in this context.
    }

    fn pop_namespace(&mut self)
    {
        // Do nothing; we don't care about namespaces in this context.
    }

    fn get_root(&mut self) -> &mut Self::Root {
        self
    }
}

/// Create parameters for a circuit, given some toxic waste.
pub fn generate_parameters<E, C>(
    circuit: C,
    g1: E::G1,
    g2: E::G2,
    alpha: E::Fr,
    beta: E::Fr,
    gamma: E::Fr,
    delta: E::Fr,
    tau: E::Fr
) -> Result<Parameters<E>, SynthesisError>
    where E: Engine, C: Circuit<E>
{
    let verbose = verbose_flag();

    let mut assembly = KeypairAssembly {
        num_inputs: 0,
        num_aux: 0,
        num_constraints: 0,
        at_inputs: vec![],
        bt_inputs: vec![],
        ct_inputs: vec![],
        at_aux: vec![],
        bt_aux: vec![],
        ct_aux: vec![]
    };

    // Allocate the "one" input variable
    assembly.alloc_input(|| "", || Ok(E::Fr::one()))?;

    // Synthesize the circuit.
    circuit.synthesize(&mut assembly)?;

    // Input constraints to ensure full density of IC query
    // x * 0 = 0
    for i in 0..assembly.num_inputs {
        assembly.enforce(|| "",
            |lc| lc + Variable(Index::Input(i)),
            |lc| lc,
            |lc| lc,
        );
    }

    if verbose {eprintln!("Making {} powers of tau", assembly.num_constraints)};
    // Create bases for blind evaluation of polynomials at tau
    let powers_of_tau = vec![Scalar::<E>(E::Fr::zero()); assembly.num_constraints];
    let mut powers_of_tau = EvaluationDomain::from_coeffs(powers_of_tau)?;

    // Compute G1 window table
    let mut g1_wnaf = Wnaf::new();
    let g1_wnaf = g1_wnaf.base(g1, {
        // H query
        (powers_of_tau.as_ref().len() - 1)
        // IC/L queries
        + assembly.num_inputs + assembly.num_aux
        // A query
        + assembly.num_inputs + assembly.num_aux
        // B query
        + assembly.num_inputs + assembly.num_aux
    });

    // Compute G2 window table
    let mut g2_wnaf = Wnaf::new();
    let g2_wnaf = g2_wnaf.base(g2, {
        // B query
        assembly.num_inputs + assembly.num_aux
    });

    let gamma_inverse = gamma.inverse().ok_or(SynthesisError::UnexpectedIdentity)?;
    let delta_inverse = delta.inverse().ok_or(SynthesisError::UnexpectedIdentity)?;

    let worker = Worker::new();

    let mut h = vec![E::G1::zero(); powers_of_tau.as_ref().len() - 1];
    {
        // Compute powers of tau
        if verbose {eprintln!("computing powers of tau...")};
        let start = PreciseTime::now();
        {
            let powers_of_tau = powers_of_tau.as_mut();
            worker.scope(powers_of_tau.len(), |scope, chunk| {
                for (i, powers_of_tau) in powers_of_tau.chunks_mut(chunk).enumerate()
                {
                    scope.spawn(move || {
                        let mut current_tau_power = tau.pow(&[(i*chunk) as u64]);

                        for p in powers_of_tau {
                            p.0 = current_tau_power;
                            current_tau_power.mul_assign(&tau);
                        }
                    });
                }
            });
        }
        if verbose {eprintln!("powers of tau stage 1 done in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);};

        // coeff = t(x) / delta
        let mut coeff = powers_of_tau.z(&tau);
        coeff.mul_assign(&delta_inverse);

        if verbose {eprintln!("computing the H query with multiple threads...")};
        let start = PreciseTime::now();
        // Compute the H query with multiple threads
        worker.scope(h.len(), |scope, chunk| {
            for (h, p) in h.chunks_mut(chunk).zip(powers_of_tau.as_ref().chunks(chunk))
            {
                let mut g1_wnaf = g1_wnaf.shared();
                scope.spawn(move || {
                    // Set values of the H query to g1^{(tau^i * t(tau)) / delta}
                    for (h, p) in h.iter_mut().zip(p.iter())
                    {
                        // Compute final exponent
                        let mut exp = p.0;
                        exp.mul_assign(&coeff);

                        // Exponentiate
                        *h = g1_wnaf.scalar(exp.into_repr());
                    }

                    // Batch normalize
                    E::G1::batch_normalization(h);
                });
            }
        });
        if verbose {eprintln!("computing the H query done in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);};
    }

    if verbose {eprintln!("using inverse FFT to convert powers of tau to Lagrange coefficients...")};
    let start = PreciseTime::now();

    // Use inverse FFT to convert powers of tau to Lagrange coefficients
    powers_of_tau.ifft(&worker);
    let powers_of_tau = powers_of_tau.into_coeffs();

    if verbose {eprintln!("powers of tau stage 2 done in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0)};

    let mut a = vec![E::G1::zero(); assembly.num_inputs + assembly.num_aux];
    let mut b_g1 = vec![E::G1::zero(); assembly.num_inputs + assembly.num_aux];
    let mut b_g2 = vec![E::G2::zero(); assembly.num_inputs + assembly.num_aux];
    let mut ic = vec![E::G1::zero(); assembly.num_inputs];
    let mut l = vec![E::G1::zero(); assembly.num_aux];

    if verbose {eprintln!("evaluating polynomials...")};
    let start = PreciseTime::now();

    fn eval<E: Engine>(
        // wNAF window tables
        g1_wnaf: &Wnaf<usize, &[E::G1], &mut Vec<i64>>,
        g2_wnaf: &Wnaf<usize, &[E::G2], &mut Vec<i64>>,

        // Lagrange coefficients for tau
        powers_of_tau: &[Scalar<E>],

        // QAP polynomials
        at: &[Vec<(E::Fr, usize)>],
        bt: &[Vec<(E::Fr, usize)>],
        ct: &[Vec<(E::Fr, usize)>],

        // Resulting evaluated QAP polynomials
        a: &mut [E::G1],
        b_g1: &mut [E::G1],
        b_g2: &mut [E::G2],
        ext: &mut [E::G1],

        // Inverse coefficient for ext elements
        inv: &E::Fr,

        // Trapdoors
        alpha: &E::Fr,
        beta: &E::Fr,

        // Worker
        worker: &Worker
    )

    {
        // Sanity check
        assert_eq!(a.len(), at.len());
        assert_eq!(a.len(), bt.len());
        assert_eq!(a.len(), ct.len());
        assert_eq!(a.len(), b_g1.len());
        assert_eq!(a.len(), b_g2.len());
        assert_eq!(a.len(), ext.len());

        // Evaluate polynomials in multiple threads
        worker.scope(a.len(), |scope, chunk| {
            for ((((((a, b_g1), b_g2), ext), at), bt), ct) in a.chunks_mut(chunk)
                                                               .zip(b_g1.chunks_mut(chunk))
                                                               .zip(b_g2.chunks_mut(chunk))
                                                               .zip(ext.chunks_mut(chunk))
                                                               .zip(at.chunks(chunk))
                                                               .zip(bt.chunks(chunk))
                                                               .zip(ct.chunks(chunk))
            {
                let mut g1_wnaf = g1_wnaf.shared();
                let mut g2_wnaf = g2_wnaf.shared();

                scope.spawn(move || {
                    for ((((((a, b_g1), b_g2), ext), at), bt), ct) in a.iter_mut()
                                                                       .zip(b_g1.iter_mut())
                                                                       .zip(b_g2.iter_mut())
                                                                       .zip(ext.iter_mut())
                                                                       .zip(at.iter())
                                                                       .zip(bt.iter())
                                                                       .zip(ct.iter())
                    {
                        fn eval_at_tau<E: Engine>(
                            powers_of_tau: &[Scalar<E>],
                            p: &[(E::Fr, usize)]
                        ) -> E::Fr
                        {
                            let mut acc = E::Fr::zero();

                            for &(ref coeff, index) in p {
                                let mut n = powers_of_tau[index].0;
                                n.mul_assign(coeff);
                                acc.add_assign(&n);
                            }

                            acc
                        }

                        // Evaluate QAP polynomials at tau
                        let mut at = eval_at_tau(powers_of_tau, at);
                        let mut bt = eval_at_tau(powers_of_tau, bt);
                        let ct = eval_at_tau(powers_of_tau, ct);

                        // Compute A query (in G1)
                        if !at.is_zero() {
                            *a = g1_wnaf.scalar(at.into_repr());
                        }

                        // Compute B query (in G1/G2)
                        if !bt.is_zero() {
                            let bt_repr = bt.into_repr();
                            *b_g1 = g1_wnaf.scalar(bt_repr);
                            *b_g2 = g2_wnaf.scalar(bt_repr);
                        }

                        at.mul_assign(&beta);
                        bt.mul_assign(&alpha);

                        let mut e = at;
                        e.add_assign(&bt);
                        e.add_assign(&ct);
                        e.mul_assign(inv);

                        *ext = g1_wnaf.scalar(e.into_repr());
                    }

                    // Batch normalize
                    E::G1::batch_normalization(a);
                    E::G1::batch_normalization(b_g1);
                    E::G2::batch_normalization(b_g2);
                    E::G1::batch_normalization(ext);
                });
            };
        });
    }

    // Evaluate for inputs.
    eval(
        &g1_wnaf,
        &g2_wnaf,
        &powers_of_tau,
        &assembly.at_inputs,
        &assembly.bt_inputs,
        &assembly.ct_inputs,
        &mut a[0..assembly.num_inputs],
        &mut b_g1[0..assembly.num_inputs],
        &mut b_g2[0..assembly.num_inputs],
        &mut ic,
        &gamma_inverse,
        &alpha,
        &beta,
        &worker
    );

    // Evaluate for auxillary variables.
    eval(
        &g1_wnaf,
        &g2_wnaf,
        &powers_of_tau,
        &assembly.at_aux,
        &assembly.bt_aux,
        &assembly.ct_aux,
        &mut a[assembly.num_inputs..],
        &mut b_g1[assembly.num_inputs..],
        &mut b_g2[assembly.num_inputs..],
        &mut l,
        &delta_inverse,
        &alpha,
        &beta,
        &worker
    );

    if verbose {eprintln!("evaluating polynomials done in {} s", start.to(PreciseTime::now()).num_milliseconds() as f64 / 1000.0);};

    // Don't allow any elements be unconstrained, so that
    // the L query is always fully dense.
    for e in l.iter() {
        if e.is_zero() {
            return Err(SynthesisError::UnconstrainedVariable);
        }
    }

    let g1 = g1.into_affine();
    let g2 = g2.into_affine();

    let vk = VerifyingKey::<E> {
        alpha_g1: g1.mul(alpha).into_affine(),
        beta_g1: g1.mul(beta).into_affine(),
        beta_g2: g2.mul(beta).into_affine(),
        gamma_g2: g2.mul(gamma).into_affine(),
        delta_g1: g1.mul(delta).into_affine(),
        delta_g2: g2.mul(delta).into_affine(),
        ic: ic.into_iter().map(|e| e.into_affine()).collect()
    };

    println!("Has generated {} points", a.len());

    Ok(Parameters {
        vk: vk,
        h: Arc::new(h.into_iter().map(|e| e.into_affine()).collect()),
        l: Arc::new(l.into_iter().map(|e| e.into_affine()).collect()),

        // Filter points at infinity away from A/B queries
        a: Arc::new(a.into_iter().filter(|e| !e.is_zero()).map(|e| e.into_affine()).collect()),
        b_g1: Arc::new(b_g1.into_iter().filter(|e| !e.is_zero()).map(|e| e.into_affine()).collect()),
        b_g2: Arc::new(b_g2.into_iter().filter(|e| !e.is_zero()).map(|e| e.into_affine()).collect())
    })
}
