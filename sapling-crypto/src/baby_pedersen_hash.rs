use babyjubjub::*;
use ff::{Field, PrimeField, PrimeFieldRepr};

#[derive(Copy, Clone)]
pub enum Personalization {
    NoteCommitment,
    MerkleTree(usize)
}

impl Personalization {
    pub fn get_bits(&self) -> Vec<bool> {
        match *self {
            Personalization::NoteCommitment =>
                vec![true, true, true, true, true, true],
            Personalization::MerkleTree(num) => {
                assert!(num < 62);

                (0..6).map(|i| (num >> i) & 1 == 1).collect()
            }
        }
    }
}

pub fn pedersen_hash<E, I>(
    personalization: Personalization,
    bits: I,
    params: &E::Params
) -> edwards::Point<E, PrimeOrder>
    where I: IntoIterator<Item=bool>,
          E: JubjubEngine
{
    let mut bits = personalization.get_bits().into_iter().chain(bits.into_iter());

    let mut result = edwards::Point::zero();
    let mut generators = params.pedersen_hash_exp_table().iter();

    loop {
        let mut acc = E::Fs::zero();
        let mut cur = E::Fs::one();
        let mut chunks_remaining = params.pedersen_hash_chunks_per_generator();
        let mut encountered_bits = false;

        // Grab three bits from the input
        while let Some(a) = bits.next() {
            encountered_bits = true;

            let b = bits.next().unwrap_or(false);
            let c = bits.next().unwrap_or(false);

            // Start computing this portion of the scalar
            let mut tmp = cur;
            if a {
                tmp.add_assign(&cur);
            }
            cur.double(); // 2^1 * cur
            if b {
                tmp.add_assign(&cur);
            }

            // conditionally negate
            if c {
                tmp.negate();
            }

            acc.add_assign(&tmp);

            chunks_remaining -= 1;

            if chunks_remaining == 0 {
                break;
            } else {
                cur.double(); // 2^2 * cur
                cur.double(); // 2^3 * cur
                cur.double(); // 2^4 * cur
            }
        }

        if !encountered_bits {
            break;
        }

        let mut table: &[Vec<edwards::Point<E, _>>] = &generators.next().expect("we don't have enough generators");
        let window = JubjubBn256::pedersen_hash_exp_window_size();
        let window_mask = (1 << window) - 1;

        let mut acc = acc.into_repr();
        
        let mut tmp = edwards::Point::zero();

        while !acc.is_zero() {
            let i = (acc.as_ref()[0] & window_mask) as usize;

            tmp = tmp.add(&table[0][i], params);

            acc.shr(window);
            table = &table[1..];
        }

        result = result.add(&tmp, params);
    }

    result
}
