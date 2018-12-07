use ff::{PrimeField, PrimeFieldRepr};

pub fn le_bit_vector_into_field_element<P: PrimeField>
    (bits: &Vec<bool>) -> P
{
    // double and add
    let mut fe = P::zero();
    let mut base = P::one();

    for bit in bits {
        if *bit {
            fe.add_assign(&base);
        }
        base.double();
    }

    fe
    // // TODO remove representation length hardcode
    // let mut bytes = [0u8; 32];

    // let byte_chunks = bits.chunks(8);

    // for (i, byte_chunk) in byte_chunks.enumerate()
    // {
    //     let mut byte = 0u8;
    //     for (j, bit) in byte_chunk.into_iter().enumerate()
    //     {
    //         if *bit {
    //             byte |= 1 << j;
    //         }
    //     }
    //     bytes[i] = byte;
    // }

    // let mut repr : P::Repr = P::zero().into_repr();
    // repr.read_le(&bytes[..]).expect("interpret as field element");

    // let field_element = P::from_repr(repr).unwrap();

    // field_element
}

pub fn be_bit_vector_into_bytes
    (bits: &Vec<bool>) -> Vec<u8>
{
    let mut bytes: Vec<u8> = vec![];

    let byte_chunks = bits.chunks(8);

    for byte_chunk in byte_chunks
    {
        let mut byte = 0u8;
        // pack just in order
        for (i, bit) in byte_chunk.into_iter().enumerate()
        {
            if *bit {
                byte |= 1 << (7 - i);
            }
        }
        bytes.push(byte);
    }

    bytes
}

pub fn le_bit_vector_into_bytes
    (bits: &Vec<bool>) -> Vec<u8>
{
    let mut bytes: Vec<u8> = vec![];

    let byte_chunks = bits.chunks(8);

    for byte_chunk in byte_chunks
    {
        let mut byte = 0u8;
        // pack just in order
        for (i, bit) in byte_chunk.into_iter().enumerate()
        {
            if *bit {
                byte |= 1 << i;
            }
        }
        bytes.push(byte);
    }

    bytes
}