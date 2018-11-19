use pairing::{Engine};
use ff::{Field};
use bellman::{ConstraintSystem, SynthesisError, Circuit, LinearCombination};
use circuit::boolean::{
    AllocatedBit,
    Boolean
};
use circuit::multipack::pack_into_inputs;

mod prfs;
mod commitment;
mod input;
mod output;

use self::input::*;
use self::output::*;

pub const TREE_DEPTH: usize = 29;

pub struct SpendingKey(pub [u8; 32]);
pub struct PayingKey(pub [u8; 32]);
pub struct UniqueRandomness(pub [u8; 32]);
pub struct CommitmentRandomness(pub [u8; 32]);

pub struct JoinSplit {
    pub vpub_old: Option<u64>,
    pub vpub_new: Option<u64>,
    pub h_sig: Option<[u8; 32]>,
    pub phi: Option<[u8; 32]>,
    pub inputs: Vec<JSInput>,
    pub outputs: Vec<JSOutput>,
    pub rt: Option<[u8; 32]>,
}

pub struct JSInput {
    pub value: Option<u64>,
    pub a_sk: Option<SpendingKey>,
    pub rho: Option<UniqueRandomness>,
    pub r: Option<CommitmentRandomness>,
    pub auth_path: [Option<([u8; 32], bool)>; TREE_DEPTH]
}

pub struct JSOutput {
    pub value: Option<u64>,
    pub a_pk: Option<PayingKey>,
    pub r: Option<CommitmentRandomness>
}

impl<E: Engine> Circuit<E> for JoinSplit {
    fn synthesize<CS: ConstraintSystem<E>>(
        self,
        cs: &mut CS
    ) -> Result<(), SynthesisError>
    {
        assert_eq!(self.inputs.len(), 2);
        assert_eq!(self.outputs.len(), 2);

        // vpub_old is the value entering the
        // JoinSplit from the "outside" value
        // pool
        let vpub_old = NoteValue::new(
            cs.namespace(|| "vpub_old"),
            self.vpub_old
        )?;

        // vpub_new is the value leaving the
        // JoinSplit into the "outside" value
        // pool
        let vpub_new = NoteValue::new(
            cs.namespace(|| "vpub_new"),
            self.vpub_new
        )?;

        // The left hand side of the balance equation
        // vpub_old + inputs[0].value + inputs[1].value
        let mut lhs = vpub_old.lc();

        // The right hand side of the balance equation
        // vpub_old + inputs[0].value + inputs[1].value
        let mut rhs = vpub_new.lc();

        // Witness rt (merkle tree root)
        let rt = witness_u256(
            cs.namespace(|| "rt"),
            self.rt.as_ref().map(|v| &v[..])
        ).unwrap();

        // Witness h_sig
        let h_sig = witness_u256(
            cs.namespace(|| "h_sig"),
            self.h_sig.as_ref().map(|v| &v[..])
        ).unwrap();

        // Witness phi
        let phi = witness_u252(
            cs.namespace(|| "phi"),
            self.phi.as_ref().map(|v| &v[..])
        ).unwrap();

        let mut input_notes = vec![];
        let mut lhs_total = self.vpub_old;

        // Iterate over the JoinSplit inputs
        for (i, input) in self.inputs.into_iter().enumerate() {
            let cs = &mut cs.namespace(|| format!("input {}", i));

            // Accumulate the value of the left hand side
            if let Some(value) = input.value {
                lhs_total = lhs_total.map(|v| v.wrapping_add(value));
            }

            // Allocate the value of the note
            let value = NoteValue::new(
                cs.namespace(|| "value"),
                input.value
            )?;

            // Compute the nonce (for PRF inputs) which is false
            // for the first input, and true for the second input.
            let nonce = match i {
                0 => false,
                1 => true,
                _ => unreachable!()
            };

            // Perform input note computations
            input_notes.push(InputNote::compute(
                cs.namespace(|| "note"),
                input.a_sk,
                input.rho,
                input.r,
                &value,
                &h_sig,
                nonce,
                input.auth_path,
                &rt
            )?);

            // Add the note value to the left hand side of
            // the balance equation
            lhs = lhs + &value.lc();
        }

        // Rebind lhs so that it isn't mutable anymore
        let lhs = lhs;

        // See zcash/zcash/issues/854
        {
            // Expected sum of the left hand side of the balance
            // equation, expressed as a 64-bit unsigned integer
            let lhs_total = NoteValue::new(
                cs.namespace(|| "total value of left hand side"),
                lhs_total
            )?;

            // Enforce that the left hand side can be expressed as a 64-bit
            // integer
            cs.enforce(
                || "left hand side can be expressed as a 64-bit unsigned integer",
                |_| lhs.clone(),
                |lc| lc + CS::one(),
                |_| lhs_total.lc()
            );
        }

        let mut output_notes = vec![];

        // Iterate over the JoinSplit outputs
        for (i, output) in self.outputs.into_iter().enumerate() {
            let cs = &mut cs.namespace(|| format!("output {}", i));

            let value = NoteValue::new(
                cs.namespace(|| "value"),
                output.value
            )?;

            // Compute the nonce (for PRF inputs) which is false
            // for the first output, and true for the second output.
            let nonce = match i {
                0 => false,
                1 => true,
                _ => unreachable!()
            };

            // Perform output note computations
            output_notes.push(OutputNote::compute(
                cs.namespace(|| "note"),
                output.a_pk,
                &value,
                output.r,
                &phi,
                &h_sig,
                nonce
            )?);

            // Add the note value to the right hand side of
            // the balance equation
            rhs = rhs + &value.lc();
        }

        // Enforce that balance is equal
        cs.enforce(
            || "balance equation",
            |_| lhs.clone(),
            |lc| lc + CS::one(),
            |_| rhs
        );

        let mut public_inputs = vec![];
        public_inputs.extend(rt);
        public_inputs.extend(h_sig);

        for note in input_notes {
            public_inputs.extend(note.nf);
            public_inputs.extend(note.mac);
        }

        for note in output_notes {
            public_inputs.extend(note.cm);
        }

        public_inputs.extend(vpub_old.bits_le());
        public_inputs.extend(vpub_new.bits_le());

        pack_into_inputs(cs.namespace(|| "input packing"), &public_inputs)
    }
}

pub struct NoteValue {
    value: Option<u64>,
    // Least significant digit first
    bits: Vec<AllocatedBit>
}

impl NoteValue {
    fn new<E, CS>(
        mut cs: CS,
        value: Option<u64>
    ) -> Result<NoteValue, SynthesisError>
        where E: Engine, CS: ConstraintSystem<E>,
    {
        let mut values;
        match value {
            Some(mut val) => {
                values = vec![];
                for _ in 0..64 {
                    values.push(Some(val & 1 == 1));
                    val >>= 1;
                }
            },
            None => {
                values = vec![None; 64];
            }
        }

        let mut bits = vec![];
        for (i, value) in values.into_iter().enumerate() {
            bits.push(
                AllocatedBit::alloc(
                    cs.namespace(|| format!("bit {}", i)),
                    value
                )?
            );
        }

        Ok(NoteValue {
            value: value,
            bits: bits
        })
    }

    /// Encodes the bits of the value into little-endian
    /// byte order.
    fn bits_le(&self) -> Vec<Boolean> {
        self.bits.chunks(8)
                 .flat_map(|v| v.iter().rev())
                 .cloned()
                 .map(|e| Boolean::from(e))
                 .collect()
    }

    /// Computes this value as a linear combination of
    /// its bits.
    fn lc<E: Engine>(&self) -> LinearCombination<E> {
        let mut tmp = LinearCombination::zero();

        let mut coeff = E::Fr::one();
        for b in &self.bits {
            tmp = tmp + (coeff, b.get_variable());
            coeff.double();
        }

        tmp
    }

    fn get_value(&self) -> Option<u64> {
        self.value
    }
}

/// Witnesses some bytes in the constraint system,
/// skipping the first `skip_bits`.
fn witness_bits<E, CS>(
    mut cs: CS,
    value: Option<&[u8]>,
    num_bits: usize,
    skip_bits: usize
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>,
{
    let bit_values = if let Some(value) = value {
        let mut tmp = vec![];
        for b in value.iter()
                      .flat_map(|&m| (0..8).rev().map(move |i| m >> i & 1 == 1))
                      .skip(skip_bits)
        {
            tmp.push(Some(b));
        }
        tmp
    } else {
        vec![None; num_bits]
    };
    assert_eq!(bit_values.len(), num_bits);

    let mut bits = vec![];

    for (i, value) in bit_values.into_iter().enumerate() {
        bits.push(Boolean::from(AllocatedBit::alloc(
            cs.namespace(|| format!("bit {}", i)),
            value
        )?));
    }

    Ok(bits)
}

fn witness_u256<E, CS>(
    cs: CS,
    value: Option<&[u8]>,
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>,
{
    witness_bits(cs, value, 256, 0)
}

fn witness_u252<E, CS>(
    cs: CS,
    value: Option<&[u8]>,
) -> Result<Vec<Boolean>, SynthesisError>
    where E: Engine, CS: ConstraintSystem<E>,
{
    witness_bits(cs, value, 252, 4)
}

#[test]
fn test_sprout_constraints() {
    use pairing::bls12_381::{Bls12};
    use ::circuit::test::*;

    use byteorder::{WriteBytesExt, ReadBytesExt, LittleEndian};

    let test_vector = include_bytes!("test_vectors.dat");
    let mut test_vector = &test_vector[..];

    fn get_u256<R: ReadBytesExt>(mut reader: R) -> [u8; 32] {
        let mut result = [0u8; 32];

        for i in 0..32 {
            result[i] = reader.read_u8().unwrap();
        }

        result
    }

    while test_vector.len() != 0 {
        let mut cs = TestConstraintSystem::<Bls12>::new();

        let phi = Some(get_u256(&mut test_vector));
        let rt = Some(get_u256(&mut test_vector));
        let h_sig = Some(get_u256(&mut test_vector));

        let mut inputs = vec![];
        for _ in 0..2 {
            test_vector.read_u8().unwrap();

            let mut auth_path = [None; TREE_DEPTH];
            for i in (0..TREE_DEPTH).rev() {
                test_vector.read_u8().unwrap();

                let sibling = get_u256(&mut test_vector);

                auth_path[i] = Some((sibling, false));
            }
            let mut position = test_vector.read_u64::<LittleEndian>().unwrap();
            for i in 0..TREE_DEPTH {
                auth_path[i].as_mut().map(|p| {
                    p.1 = (position & 1) == 1
                });

                position >>= 1;
            }

            // a_pk
            let _ = Some(SpendingKey(get_u256(&mut test_vector)));
            let value = Some(test_vector.read_u64::<LittleEndian>().unwrap());
            let rho = Some(UniqueRandomness(get_u256(&mut test_vector)));
            let r = Some(CommitmentRandomness(get_u256(&mut test_vector)));
            let a_sk = Some(SpendingKey(get_u256(&mut test_vector)));

            inputs.push(
                JSInput {
                    value: value,
                    a_sk: a_sk,
                    rho: rho,
                    r: r,
                    auth_path: auth_path
                }
            );
        }

        let mut outputs = vec![];

        for _ in 0..2 {
            let a_pk = Some(PayingKey(get_u256(&mut test_vector)));
            let value = Some(test_vector.read_u64::<LittleEndian>().unwrap());
            get_u256(&mut test_vector);
            let r = Some(CommitmentRandomness(get_u256(&mut test_vector)));

            outputs.push(
                JSOutput {
                    value: value,
                    a_pk: a_pk,
                    r: r
                }
            );
        }

        let vpub_old = Some(test_vector.read_u64::<LittleEndian>().unwrap());
        let vpub_new = Some(test_vector.read_u64::<LittleEndian>().unwrap());

        let nf1 = get_u256(&mut test_vector);
        let nf2 = get_u256(&mut test_vector);

        let cm1 = get_u256(&mut test_vector);
        let cm2 = get_u256(&mut test_vector);

        let mac1 = get_u256(&mut test_vector);
        let mac2 = get_u256(&mut test_vector);

        let js = JoinSplit {
            vpub_old: vpub_old,
            vpub_new: vpub_new,
            h_sig: h_sig,
            phi: phi,
            inputs: inputs,
            outputs: outputs,
            rt: rt
        };

        js.synthesize(&mut cs).unwrap();

        if let Some(s) = cs.which_is_unsatisfied() {
            panic!("{:?}", s);
        }
        assert!(cs.is_satisfied());
        assert_eq!(cs.num_constraints(), 1989085);
        assert_eq!(cs.num_inputs(), 10);
        assert_eq!(cs.hash(), "1a228d3c6377130d1778c7885811dc8b8864049cb5af8aff7e6cd46c5bc4b84c");

        let mut expected_inputs = vec![];
        expected_inputs.extend(rt.unwrap().to_vec());
        expected_inputs.extend(h_sig.unwrap().to_vec());
        expected_inputs.extend(nf1.to_vec());
        expected_inputs.extend(mac1.to_vec());
        expected_inputs.extend(nf2.to_vec());
        expected_inputs.extend(mac2.to_vec());
        expected_inputs.extend(cm1.to_vec());
        expected_inputs.extend(cm2.to_vec());
        expected_inputs.write_u64::<LittleEndian>(vpub_old.unwrap()).unwrap();
        expected_inputs.write_u64::<LittleEndian>(vpub_new.unwrap()).unwrap();

        use circuit::multipack;

        let expected_inputs = multipack::bytes_to_bits(&expected_inputs);
        let expected_inputs = multipack::compute_multipacking::<Bls12>(&expected_inputs);

        assert!(cs.verify(&expected_inputs));
    }
}
