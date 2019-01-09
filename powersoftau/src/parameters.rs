extern crate pairing;
extern crate rand;
extern crate crossbeam;
extern crate num_cpus;
extern crate blake2;
extern crate generic_array;
extern crate typenum;
extern crate byteorder;
extern crate ff;

use self::ff::{Field, PrimeField};
use self::byteorder::{ReadBytesExt, BigEndian};
use self::rand::{SeedableRng, Rng, Rand};
use self::rand::chacha::ChaChaRng;
use self::pairing::bn256::{Bn256};
use self::pairing::*;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use self::generic_array::GenericArray;
use self::typenum::consts::U64;
use self::blake2::{Blake2b, Digest};
use std::fmt;

use super::keypair::*;

pub trait PowersOfTauParameters: Clone {
    const REQUIRED_POWER: usize; 
    
    const G1_UNCOMPRESSED_BYTE_SIZE: usize;
    const G2_UNCOMPRESSED_BYTE_SIZE: usize;
    const G1_COMPRESSED_BYTE_SIZE: usize;
    const G2_COMPRESSED_BYTE_SIZE: usize;

    const TAU_POWERS_LENGTH: usize = (1 << Self::REQUIRED_POWER);

    const TAU_POWERS_G1_LENGTH: usize = (Self::TAU_POWERS_LENGTH << 1) - 1;

    const ACCUMULATOR_BYTE_SIZE: usize = (Self::TAU_POWERS_G1_LENGTH * Self::G1_UNCOMPRESSED_BYTE_SIZE) + // g1 tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G2_UNCOMPRESSED_BYTE_SIZE) + // g2 tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G1_UNCOMPRESSED_BYTE_SIZE) + // alpha tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G1_UNCOMPRESSED_BYTE_SIZE) // beta tau powers
                                            + Self::G2_UNCOMPRESSED_BYTE_SIZE // beta in g2
                                            + Self::HASH_SIZE; // blake2b hash of previous contribution

    const PUBLIC_KEY_SIZE: usize = 3 * Self::G2_UNCOMPRESSED_BYTE_SIZE + // tau, alpha, and beta in g2
                                    6 * Self::G1_UNCOMPRESSED_BYTE_SIZE; // (s1, s1*tau), (s2, s2*alpha), (s3, s3*beta) in g1

    const CONTRIBUTION_BYTE_SIZE: usize = (Self::TAU_POWERS_G1_LENGTH * Self::G1_COMPRESSED_BYTE_SIZE) + // g1 tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G2_COMPRESSED_BYTE_SIZE) + // g2 tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G1_COMPRESSED_BYTE_SIZE) + // alpha tau powers
                                            (Self::TAU_POWERS_LENGTH * Self::G1_COMPRESSED_BYTE_SIZE) // beta tau powers
                                            + Self::G2_COMPRESSED_BYTE_SIZE // beta in g2
                                            + Self::HASH_SIZE // blake2b hash of input accumulator
                                            + Self::PUBLIC_KEY_SIZE; // public key

    // Blake2b hash size
    const HASH_SIZE: usize = 64;

    const EMPIRICAL_BATCH_SIZE: usize = 1 << 21;
}



/// Determines if point compression should be used.
#[derive(Copy, Clone, PartialEq)]
pub enum UseCompression {
    Yes,
    No
}

/// Determines if points should be checked for correctness during deserialization.
/// This is not necessary for participants, because a transcript verifier can
/// check this theirself.
#[derive(Copy, Clone, PartialEq)]
pub enum CheckForCorrectness {
    Yes,
    No
}


/// Errors that might occur during deserialization.
#[derive(Debug)]
pub enum DeserializationError {
    IoError(io::Error),
    DecodingError(GroupDecodingError),
    PointAtInfinity
}

impl fmt::Display for DeserializationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DeserializationError::IoError(ref e) => write!(f, "Disk IO error: {}", e),
            DeserializationError::DecodingError(ref e) => write!(f, "Decoding error: {}", e),
            DeserializationError::PointAtInfinity => write!(f, "Point at infinity found")
        }
    }
}

impl From<io::Error> for DeserializationError {
    fn from(err: io::Error) -> DeserializationError {
        DeserializationError::IoError(err)
    }
}

impl From<GroupDecodingError> for DeserializationError {
    fn from(err: GroupDecodingError) -> DeserializationError {
        DeserializationError::DecodingError(err)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ElementType {
    TauG1,
    TauG2,
    AlphaG1,
    BetaG1,
    BetaG2
}