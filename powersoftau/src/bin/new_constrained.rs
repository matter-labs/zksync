extern crate powersoftau;
extern crate pairing;
extern crate memmap;

// use powersoftau::bn256::{Bn256CeremonyParameters};
use powersoftau::small_bn256::{Bn256CeremonyParameters};
use powersoftau::batched_accumulator::{BachedAccumulator};
use powersoftau::parameters::{UseCompression};
use powersoftau::utils::{blank_hash};

use std::fs::OpenOptions;
use std::io::{Write};
use pairing::bn256::Bn256;
use memmap::*;

use powersoftau::parameters::PowersOfTauParameters;

const compress_new_challenge: UseCompression = UseCompression::No;

fn main() {
    println!("Will generate an empty accumulator for 2^{} powers of tau", Bn256CeremonyParameters::REQUIRED_POWER);
    println!("In total will generate up to {} powers", Bn256CeremonyParameters::TAU_POWERS_G1_LENGTH);
    
    let file = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create_new(true)
                            .open("challenge").expect("unable to create `./challenge`");
            
    let expected_challenge_length = match compress_new_challenge {
        UseCompression::Yes => {
            Bn256CeremonyParameters::CONTRIBUTION_BYTE_SIZE - Bn256CeremonyParameters::PUBLIC_KEY_SIZE
        },
        UseCompression::No => {
            Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE
        }
    };

    file.set_len(expected_challenge_length as u64).expect("unable to allocate large enough file");

    let mut writable_map = unsafe { MmapOptions::new().map_mut(&file).expect("unable to create a memory map") };

    // Write a blank BLAKE2b hash:
    let hash = blank_hash();
    (&mut writable_map[0..]).write(hash.as_slice()).expect("unable to write a default hash to mmap");
    writable_map.flush().expect("unable to write blank hash to `./challenge`");

    println!("Blank hash for an empty challenge:");
    for line in hash.as_slice().chunks(16) {
        print!("\t");
        for section in line.chunks(4) {
            for b in section {
                print!("{:02x}", b);
            }
            print!(" ");
        }
        println!("");
    }

    BachedAccumulator::<Bn256, Bn256CeremonyParameters>::generate_initial(&mut writable_map, compress_new_challenge).expect("generation of initial accumulator is successful");
    writable_map.flush().expect("unable to flush memmap to disk");

    // Get the hash of the contribution, so the user can compare later
    let output_readonly = writable_map.make_read_only().expect("must make a map readonly");
    let contribution_hash = BachedAccumulator::<Bn256, Bn256CeremonyParameters>::calculate_hash(&output_readonly);

    println!("Empty contribution is formed with a hash:");

    for line in contribution_hash.as_slice().chunks(16) {
        print!("\t");
        for section in line.chunks(4) {
            for b in section {
                print!("{:02x}", b);
            }
            print!(" ");
        }
        println!("");
    }

    println!("Wrote a fresh accumulator to `./challenge`");
}
