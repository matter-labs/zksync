use pairing::{
    Engine,
};

use bellman::{
    SynthesisError,
    ConstraintSystem
};

use super::boolean::{
    Boolean
};

use super::uint32::{
    UInt32
};

use super::multieq::MultiEq;

/*
2.1.  Parameters
   The following table summarizes various parameters and their ranges:
                            | BLAKE2b          | BLAKE2s          |
              --------------+------------------+------------------+
               Bits in word | w = 64           | w = 32           |
               Rounds in F  | r = 12           | r = 10           |
               Block bytes  | bb = 128         | bb = 64          |
               Hash bytes   | 1 <= nn <= 64    | 1 <= nn <= 32    |
               Key bytes    | 0 <= kk <= 64    | 0 <= kk <= 32    |
               Input bytes  | 0 <= ll < 2**128 | 0 <= ll < 2**64  |
              --------------+------------------+------------------+
               G Rotation   | (R1, R2, R3, R4) | (R1, R2, R3, R4) |
                constants = | (32, 24, 16, 63) | (16, 12,  8,  7) |
              --------------+------------------+------------------+
*/

const R1: usize = 16;
const R2: usize = 12;
const R3: usize = 8;
const R4: usize = 7;

/*
          Round   |  0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15 |
        ----------+-------------------------------------------------+
         SIGMA[0] |  0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15 |
         SIGMA[1] | 14 10  4  8  9 15 13  6  1 12  0  2 11  7  5  3 |
         SIGMA[2] | 11  8 12  0  5  2 15 13 10 14  3  6  7  1  9  4 |
         SIGMA[3] |  7  9  3  1 13 12 11 14  2  6  5 10  4  0 15  8 |
         SIGMA[4] |  9  0  5  7  2  4 10 15 14  1 11 12  6  8  3 13 |
         SIGMA[5] |  2 12  6 10  0 11  8  3  4 13  7  5 15 14  1  9 |
         SIGMA[6] | 12  5  1 15 14 13  4 10  0  7  6  3  9  2  8 11 |
         SIGMA[7] | 13 11  7 14 12  1  3  9  5  0 15  4  8  6  2 10 |
         SIGMA[8] |  6 15 14  9 11  3  0  8 12  2 13  7  1  4 10  5 |
         SIGMA[9] | 10  2  8  4  7  6  1  5 15 11  9 14  3 12 13  0 |
        ----------+-------------------------------------------------+
*/

const SIGMA: [[usize; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0]
];

/*
3.1.  Mixing Function G
   The G primitive function mixes two input words, "x" and "y", into
   four words indexed by "a", "b", "c", and "d" in the working vector
   v[0..15].  The full modified vector is returned.  The rotation
   constants (R1, R2, R3, R4) are given in Section 2.1.
       FUNCTION G( v[0..15], a, b, c, d, x, y )
       |
       |   v[a] := (v[a] + v[b] + x) mod 2**w
       |   v[d] := (v[d] ^ v[a]) >>> R1
       |   v[c] := (v[c] + v[d])     mod 2**w
       |   v[b] := (v[b] ^ v[c]) >>> R2
       |   v[a] := (v[a] + v[b] + y) mod 2**w
       |   v[d] := (v[d] ^ v[a]) >>> R3
       |   v[c] := (v[c] + v[d])     mod 2**w
       |   v[b] := (v[b] ^ v[c]) >>> R4
       |
       |   RETURN v[0..15]
       |
       END FUNCTION.
*/

fn mixing_g<E: Engine, CS: ConstraintSystem<E>, M>(
    mut cs: M,
    v: &mut [UInt32],
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    x: &UInt32,
    y: &UInt32
) -> Result<(), SynthesisError>
    where M: ConstraintSystem<E, Root=MultiEq<E, CS>>
{
    v[a] = UInt32::addmany(cs.namespace(|| "mixing step 1"), &[v[a].clone(), v[b].clone(), x.clone()])?;
    v[d] = v[d].xor(cs.namespace(|| "mixing step 2"), &v[a])?.rotr(R1);
    v[c] = UInt32::addmany(cs.namespace(|| "mixing step 3"), &[v[c].clone(), v[d].clone()])?;
    v[b] = v[b].xor(cs.namespace(|| "mixing step 4"), &v[c])?.rotr(R2);
    v[a] = UInt32::addmany(cs.namespace(|| "mixing step 5"), &[v[a].clone(), v[b].clone(), y.clone()])?;
    v[d] = v[d].xor(cs.namespace(|| "mixing step 6"), &v[a])?.rotr(R3);
    v[c] = UInt32::addmany(cs.namespace(|| "mixing step 7"), &[v[c].clone(), v[d].clone()])?;
    v[b] = v[b].xor(cs.namespace(|| "mixing step 8"), &v[c])?.rotr(R4);

    Ok(())
}

/*
3.2.  Compression Function F
   Compression function F takes as an argument the state vector "h",
   message block vector "m" (last block is padded with zeros to full
   block size, if required), 2w-bit offset counter "t", and final block
   indicator flag "f".  Local vector v[0..15] is used in processing.  F
   returns a new state vector.  The number of rounds, "r", is 12 for
   BLAKE2b and 10 for BLAKE2s.  Rounds are numbered from 0 to r - 1.
       FUNCTION F( h[0..7], m[0..15], t, f )
       |
       |      // Initialize local work vector v[0..15]
       |      v[0..7] := h[0..7]              // First half from state.
       |      v[8..15] := IV[0..7]            // Second half from IV.
       |
       |      v[12] := v[12] ^ (t mod 2**w)   // Low word of the offset.
       |      v[13] := v[13] ^ (t >> w)       // High word.
       |
       |      IF f = TRUE THEN                // last block flag?
       |      |   v[14] := v[14] ^ 0xFF..FF   // Invert all bits.
       |      END IF.
       |
       |      // Cryptographic mixing
       |      FOR i = 0 TO r - 1 DO           // Ten or twelve rounds.
       |      |
       |      |   // Message word selection permutation for this round.
       |      |   s[0..15] := SIGMA[i mod 10][0..15]
       |      |
       |      |   v := G( v, 0, 4,  8, 12, m[s[ 0]], m[s[ 1]] )
       |      |   v := G( v, 1, 5,  9, 13, m[s[ 2]], m[s[ 3]] )
       |      |   v := G( v, 2, 6, 10, 14, m[s[ 4]], m[s[ 5]] )
       |      |   v := G( v, 3, 7, 11, 15, m[s[ 6]], m[s[ 7]] )
       |      |
       |      |   v := G( v, 0, 5, 10, 15, m[s[ 8]], m[s[ 9]] )
       |      |   v := G( v, 1, 6, 11, 12, m[s[10]], m[s[11]] )
       |      |   v := G( v, 2, 7,  8, 13, m[s[12]], m[s[13]] )
       |      |   v := G( v, 3, 4,  9, 14, m[s[14]], m[s[15]] )
       |      |
       |      END FOR
       |
       |      FOR i = 0 TO 7 DO               // XOR the two halves.
       |      |   h[i] := h[i] ^ v[i] ^ v[i + 8]
       |      END FOR.
       |
       |      RETURN h[0..7]                  // New state.
       |
       END FUNCTION.
*/


fn blake2s_compression<E: Engine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    h: &mut [UInt32],
    m: &[UInt32],
    t: u64,
    f: bool
) -> Result<(), SynthesisError>
{
    assert_eq!(h.len(), 8);
    assert_eq!(m.len(), 16);

    /*
    static const uint32_t blake2s_iv[8] =
    {
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
        0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19
    };
    */

    let mut v = Vec::with_capacity(16);
    v.extend_from_slice(h);
    v.push(UInt32::constant(0x6A09E667));
    v.push(UInt32::constant(0xBB67AE85));
    v.push(UInt32::constant(0x3C6EF372));
    v.push(UInt32::constant(0xA54FF53A));
    v.push(UInt32::constant(0x510E527F));
    v.push(UInt32::constant(0x9B05688C));
    v.push(UInt32::constant(0x1F83D9AB));
    v.push(UInt32::constant(0x5BE0CD19));

    assert_eq!(v.len(), 16);

    v[12] = v[12].xor(cs.namespace(|| "first xor"), &UInt32::constant(t as u32))?;
    v[13] = v[13].xor(cs.namespace(|| "second xor"), &UInt32::constant((t >> 32) as u32))?;

    if f {
        v[14] = v[14].xor(cs.namespace(|| "third xor"), &UInt32::constant(u32::max_value()))?;
    }

    {
        let mut cs = MultiEq::new(&mut cs);

        for i in 0..10 {
            let mut cs = cs.namespace(|| format!("round {}", i));

            let s = SIGMA[i % 10];

            mixing_g(cs.namespace(|| "mixing invocation 1"), &mut v, 0, 4,  8, 12, &m[s[ 0]], &m[s[ 1]])?;
            mixing_g(cs.namespace(|| "mixing invocation 2"), &mut v, 1, 5,  9, 13, &m[s[ 2]], &m[s[ 3]])?;
            mixing_g(cs.namespace(|| "mixing invocation 3"), &mut v, 2, 6, 10, 14, &m[s[ 4]], &m[s[ 5]])?;
            mixing_g(cs.namespace(|| "mixing invocation 4"), &mut v, 3, 7, 11, 15, &m[s[ 6]], &m[s[ 7]])?;

            mixing_g(cs.namespace(|| "mixing invocation 5"), &mut v, 0, 5, 10, 15, &m[s[ 8]], &m[s[ 9]])?;
            mixing_g(cs.namespace(|| "mixing invocation 6"), &mut v, 1, 6, 11, 12, &m[s[10]], &m[s[11]])?;
            mixing_g(cs.namespace(|| "mixing invocation 7"), &mut v, 2, 7,  8, 13, &m[s[12]], &m[s[13]])?;
            mixing_g(cs.namespace(|| "mixing invocation 8"), &mut v, 3, 4,  9, 14, &m[s[14]], &m[s[15]])?;
        }
    }

    for i in 0..8 {
        let mut cs = cs.namespace(|| format!("h[{i}] ^ v[{i}] ^ v[{i} + 8]", i=i));

        h[i] = h[i].xor(cs.namespace(|| "first xor"), &v[i])?;
        h[i] = h[i].xor(cs.namespace(|| "second xor"), &v[i + 8])?;
    }

    Ok(())
}

/*
        FUNCTION BLAKE2( d[0..dd-1], ll, kk, nn )
        |
        |     h[0..7] := IV[0..7]          // Initialization Vector.
        |
        |     // Parameter block p[0]
        |     h[0] := h[0] ^ 0x01010000 ^ (kk << 8) ^ nn
        |
        |     // Process padded key and data blocks
        |     IF dd > 1 THEN
        |     |       FOR i = 0 TO dd - 2 DO
        |     |       |       h := F( h, d[i], (i + 1) * bb, FALSE )
        |     |       END FOR.
        |     END IF.
        |
        |     // Final block.
        |     IF kk = 0 THEN
        |     |       h := F( h, d[dd - 1], ll, TRUE )
        |     ELSE
        |     |       h := F( h, d[dd - 1], ll + bb, TRUE )
        |     END IF.
        |
        |     RETURN first "nn" bytes from little-endian word array h[].
        |
        END FUNCTION.
*/

pub fn blake2s<E: Engine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    input: &[Boolean],
    personalization: &[u8]
) -> Result<Vec<Boolean>, SynthesisError>
{
    use byteorder::{ByteOrder, LittleEndian};

    assert_eq!(personalization.len(), 8);
    assert!(input.len() % 8 == 0);

    let mut h = Vec::with_capacity(8);
    h.push(UInt32::constant(0x6A09E667 ^ 0x01010000 ^ 32));
    h.push(UInt32::constant(0xBB67AE85));
    h.push(UInt32::constant(0x3C6EF372));
    h.push(UInt32::constant(0xA54FF53A));
    h.push(UInt32::constant(0x510E527F));
    h.push(UInt32::constant(0x9B05688C));

    // Personalization is stored here
    h.push(UInt32::constant(0x1F83D9AB ^ LittleEndian::read_u32(&personalization[0..4])));
    h.push(UInt32::constant(0x5BE0CD19 ^ LittleEndian::read_u32(&personalization[4..8])));

    let mut blocks: Vec<Vec<UInt32>> = vec![];

    for block in input.chunks(512) {
        let mut this_block = Vec::with_capacity(16);
        for word in block.chunks(32) {
            let mut tmp = word.to_vec();
            while tmp.len() < 32 {
                tmp.push(Boolean::constant(false));
            }
            this_block.push(UInt32::from_bits(&tmp));
        }
        while this_block.len() < 16 {
            this_block.push(UInt32::constant(0));
        }
        blocks.push(this_block);
    }

    if blocks.len() == 0 {
        blocks.push((0..16).map(|_| UInt32::constant(0)).collect());
    }

    for (i, block) in blocks[0..blocks.len() - 1].iter().enumerate() {
        let cs = cs.namespace(|| format!("block {}", i));

        blake2s_compression(cs, &mut h, block, ((i as u64) + 1) * 64, false)?;
    }

    {
        let cs = cs.namespace(|| "final block");

        blake2s_compression(cs, &mut h, &blocks[blocks.len() - 1], (input.len() / 8) as u64, true)?;
    }

    Ok(h.iter().flat_map(|b| b.into_bits()).collect())
}

#[cfg(test)]
mod test {
    use rand::{XorShiftRng, SeedableRng, Rng};
    use pairing::bls12_381::{Bls12};
    use ::circuit::boolean::{Boolean, AllocatedBit};
    use ::circuit::test::TestConstraintSystem;
    use super::blake2s;
    use bellman::{ConstraintSystem};
    use blake2_rfc::blake2s::Blake2s;

    #[test]
    fn test_blank_hash() {
        let mut cs = TestConstraintSystem::<Bls12>::new();
        let input_bits = vec![];
        let out = blake2s(&mut cs, &input_bits, b"12345678").unwrap();
        assert!(cs.is_satisfied());
        assert_eq!(cs.num_constraints(), 0);

        // >>> import blake2s from hashlib
        // >>> h = blake2s(digest_size=32, person=b'12345678')
        // >>> h.hexdigest()
        let expected = hex!("c59f682376d137f3f255e671e207d1f2374ebe504e9314208a52d9f88d69e8c8");

        let mut out = out.into_iter();
        for b in expected.into_iter() {
            for i in 0..8 {
                let c = out.next().unwrap().get_value().unwrap();

                assert_eq!(c, (b >> i) & 1u8 == 1u8);
            }
        }
    }

    #[test]
    fn test_blake2s_constraints() {
        let mut cs = TestConstraintSystem::<Bls12>::new();
        let input_bits: Vec<_> = (0..512).map(|i| AllocatedBit::alloc(cs.namespace(|| format!("input bit {}", i)), Some(true)).unwrap().into()).collect();
        blake2s(&mut cs, &input_bits, b"12345678").unwrap();
        assert!(cs.is_satisfied());
        assert_eq!(cs.num_constraints(), 21518);
    }

    #[test]
    fn test_blake2s_precomp_constraints() {
        // Test that 512 fixed leading bits (constants)
        // doesn't result in more constraints.

        let mut cs = TestConstraintSystem::<Bls12>::new();
        let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let input_bits: Vec<_> = (0..512)
          .map(|_| Boolean::constant(rng.gen()))
          .chain((0..512)
                        .map(|i| AllocatedBit::alloc(cs.namespace(|| format!("input bit {}", i)), Some(true)).unwrap().into()))
          .collect();
        blake2s(&mut cs, &input_bits, b"12345678").unwrap();
        assert!(cs.is_satisfied());
        assert_eq!(cs.num_constraints(), 21518);
    }

    #[test]
    fn test_blake2s_constant_constraints() {
        let mut cs = TestConstraintSystem::<Bls12>::new();
        let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
        let input_bits: Vec<_> = (0..512).map(|_| Boolean::constant(rng.gen())).collect();
        blake2s(&mut cs, &input_bits, b"12345678").unwrap();
        assert_eq!(cs.num_constraints(), 0);
    }

    #[test]
    fn test_blake2s() {
        let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

        for input_len in (0..32).chain((32..256).filter(|a| a % 8 == 0))
        {
            let mut h = Blake2s::with_params(32, &[], &[], b"12345678");

            let data: Vec<u8> = (0..input_len).map(|_| rng.gen()).collect();

            h.update(&data);

            let hash_result = h.finalize();

            let mut cs = TestConstraintSystem::<Bls12>::new();

            let mut input_bits = vec![];

            for (byte_i, input_byte) in data.into_iter().enumerate() {
                for bit_i in 0..8 {
                    let cs = cs.namespace(|| format!("input bit {} {}", byte_i, bit_i));

                    input_bits.push(AllocatedBit::alloc(cs, Some((input_byte >> bit_i) & 1u8 == 1u8)).unwrap().into());
                }
            }

            let r = blake2s(&mut cs, &input_bits, b"12345678").unwrap();

            assert!(cs.is_satisfied());

            let mut s = hash_result.as_ref().iter()
                                            .flat_map(|&byte| (0..8).map(move |i| (byte >> i) & 1u8 == 1u8));

            for b in r {
                match b {
                    Boolean::Is(b) => {
                        assert!(s.next().unwrap() == b.get_value().unwrap());
                    },
                    Boolean::Not(b) => {
                        assert!(s.next().unwrap() != b.get_value().unwrap());
                    },
                    Boolean::Constant(b) => {
                        assert!(input_len == 0);
                        assert!(s.next().unwrap() == b);
                    }
                }
            }
        }
    }
}
