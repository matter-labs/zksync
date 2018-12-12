// This is an attempt to write an implementation of Dmitry Khovratovich

use pairing::{Engine};
use bellman::{ConstraintSystem, SynthesisError};
use rand::Rng;
use ff::{Field, PrimeField, PrimeFieldRepr, BitIterator};

use super::*;

const block_size: usize = 256;
const gate_size: usize = 32;
const branch_size: usize = 32;
const num_branches: usize = 4;
const middle_rounds: usize = 38;
const total_rounds: usize = 3 + middle_rounds + 3;
const num_round_keys: usize = (middle_rounds + 7) * num_branches;
const num_round_constants: usize = (middle_rounds + 6) * num_branches;

pub struct SharkMimc<E: Engine> {
    // round_constants : [E::Fr; num_round_constants],
    round_keys:  [E::Fr; num_round_keys],
    matrix_1: [[E::Fr; num_branches]; num_branches],
    matrix_2: [[E::Fr; num_branches]; num_branches],
    // linear_vals : Vec<num::AllocatedNum<E>>,
    // round_squares : Vec<num::AllocatedNum<E>>,
    // sbox_outs: Vec<num::AllocatedNum<E>>,
}

impl<E:Engine> SharkMimc<E> {
    fn new<R, CS>(
        mut cs: CS,
        rng: &mut R
    ) -> Self
        where CS: ConstraintSystem<E>, R: Rng
    {
        // generate round keys
        let mut round_keys = [E::Fr::zero(); num_round_keys];
        for i in 0..num_round_keys {
            let random_element: E::Fr = rng.gen();
            round_keys[i] = random_element;
        }

        // prepare round constants
        // let mut round_constants = [E::Fr::zero(); num_round_constants];
        // for i in 0..num_round_constants {
        //     let random_element: E::Fr = rng.gen();
        //     round_keys[i] = random_element;
        // }

        let mut matrix_1 = [[E::Fr::one(); num_branches]; num_branches];

        {
            let x = [E::Fr::from_str(&"1").unwrap(),
                    E::Fr::from_str(&"2").unwrap(),
                    E::Fr::from_str(&"3").unwrap(),
                    E::Fr::from_str(&"4").unwrap()
                ];

            let y = [E::Fr::from_str(&"5").unwrap(),
                    E::Fr::from_str(&"6").unwrap(),
                    E::Fr::from_str(&"7").unwrap(),
                    E::Fr::from_str(&"8").unwrap()
                ];

            let one = E::Fr::one();

            // Char - 2
            let mut power = E::Fr::zero();
            power.sub_assign(&one);
            power.sub_assign(&one);
            
            let mut element = E::Fr::zero();
            let mut base_temp = E::Fr::zero();
            let mut exp_temp = E::Fr::zero;
            for i in 0..num_branches {
                for j in 0..num_branches {
                    let mut element = x[i];
                    element.add_assign(&y[j]);
                    element.pow(power.into_repr());

                    // let mut bit_iterator: Vec<bool> = BitIterator::new(element.into_repr()).collect();
                    // bit_iterator.reverse();
                    // let mut res = E::Fr::one();
                    // for bit in bit_iterator.into_iter() {
                    //     if bit {
                    //         res.mul_assign(&element);
                    //     }
                    //     res.square();
                    // }

                    matrix_1[i][j] = element
                }
            }
        }

        let mut matrix_2 = [[E::Fr::one(); num_branches]; num_branches];

        {
            let x = [E::Fr::from_str(&"9").unwrap(),
                    E::Fr::from_str(&"10").unwrap(),
                    E::Fr::from_str(&"11").unwrap(),
                    E::Fr::from_str(&"12").unwrap()
                ];

            let y = [E::Fr::from_str(&"13").unwrap(),
                    E::Fr::from_str(&"14").unwrap(),
                    E::Fr::from_str(&"15").unwrap(),
                    E::Fr::from_str(&"16").unwrap()
                ];

            let one = E::Fr::one();

            // Char - 2
            let mut power = E::Fr::zero();
            power.sub_assign(&one);
            power.sub_assign(&one);
            
            let mut element = E::Fr::zero();
            let mut base_temp = E::Fr::zero();
            let mut exp_temp = E::Fr::zero;
            for i in 0..num_branches {
                for j in 0..num_branches {
                    let mut element = x[i];
                    element.add_assign(&y[j]);
                    element.pow(power.into_repr());

                    // let mut bit_iterator: Vec<bool> = BitIterator::new(element.into_repr()).collect();
                    // bit_iterator.reverse();
                    // let mut res = E::Fr::one();
                    // for bit in bit_iterator.into_iter() {
                    //     if bit {
                    //         res.mul_assign(&element);
                    //     }
                    //     res.square();
                    // }

                    matrix_2[i][j] = element
                }
            }
        }

        Self {
            // round_constants : round_constants,
            round_keys:  round_keys,
            matrix_1: matrix_1,
            matrix_2: matrix_2,
            // linear_vals : Vec<num::AllocatedNum<E>>,
            // round_squares : Vec<num::AllocatedNum<E>>,
            // sbox_outs: Vec<num::AllocatedNum<E>>,
        }
    }

    fn hash<CS>(
        &self,
        mut cs: CS,
        inputs: &[num::AllocatedNum<E>]
    ) -> Result<num::AllocatedNum<E>, SynthesisError> 
        where CS: ConstraintSystem<E>
    {
        // Ok, idea is to do the chain
        // M of 
        // - full sbox
        // - affine transformation
        // N of
        // - signle sbox
        // - affine transformations
        // M of 
        // - full sbox
        // - affine transformation

        assert_eq!(inputs.len(), num_branches);
        let cs = cs.namespace(|| "Sharkmimc inverse gadget");
        let M = 6;
        let N = 14;

        let mut ins = inputs;
        let mut outs = vec![];

        // invert and witnessize input variables (t1, t2, t3, t4 -> t1^-1, t2^-1, t3^-1, t4^-1)


        for branch in 0..branch_size {
            let input = ins[branch];
            let s_box_out = num::AllocatedNum::alloc(
                cs.namespace(|| format!("Allocate sbox output for round number {}, branch {}", 0, branch)), 
                || {
                    let t = input.get_value().get()?;
                    match t.inverse() {
                            Some(t) => {
                                Ok(t)
                            },
                        None => {
                            Err(SynthesisError::DivisionByZero)
                        }
                    }
                }   
            )?;

            cs.enforce(
                || format!("s box for round {} computation, branch {}", 0, branch),
                |lc| lc + input.get_variable(),
                |lc| lc + s_box_out.get_variable(),
                |lc| lc + CS::one()
            );

            outs.push(s_box_out);
        }

        for round_number in 1..M {
            ins = outs;
            outs = vec![];
            // now it's more tricky - combine the affine transformation and the next s-box
            for branch in 0..branch_size {
                let s_box_out = num::AllocatedNum::alloc(
                    cs.namespace(|| format!("Allocate sbox output for round number {}, branch {}", round_number, branch)), 
                    || {
                        let mut t = self.round_keys[M*(round_number-1) + branch];
                        let i in 0..num_branches {
                            let input = ins[i];
                            let mut multiplication = input.get_value().get()?;
                            multiplication.mul_assign(&matrix_1[branch][i]);
                            t.add_assign(&multiplication);
                        }
                        // this is a linear combination (c11 + a11 * t1^-1 + a12 * t12^-1 + a13 * t3^-1 + a14 * t4^-1)
                        match t.inverse() {
                                Some(t) => {
                                    Ok(t)
                                },
                            None => {
                                Err(SynthesisError::DivisionByZero)
                            }
                        }
                    }   
                )?;

                cs.enforce(
                    || format!("affine + s box for round {} computation, branch {}", round_number, branch),
                    |lc| {
                        let mut l = lc + (self.round_keys[M*(round_number-1) + branch], CS::one());
                        let i in 0..num_branches {
                            let input = ins[i];
                            l = l + (matrix_1[branch][i], input.get_variable());                            
                        }

                        l
                    },
                    |lc| lc + s_box_out.get_variable(),
                    |lc| lc + CS::one()
                );

                outs.push(s_box_out);
            }
        }

        out[num_branches - 1]
    }
}