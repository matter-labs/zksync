extern crate powersoftau;
extern crate pairing;
extern crate memmap;
extern crate rand;
extern crate blake2;
extern crate byteorder;

// use powersoftau::bn256::{Bn256CeremonyParameters};
use powersoftau::small_bn256::{Bn256CeremonyParameters};
use powersoftau::batched_accumulator::{BachedAccumulator};
use powersoftau::keypair::{PublicKey};
use powersoftau::parameters::{UseCompression, CheckForCorrectness};

use std::fs::OpenOptions;
use pairing::bn256::Bn256;
use memmap::*;

use std::io::{Read, Write};

use powersoftau::parameters::PowersOfTauParameters;

const previous_challenge_is_compressed: UseCompression = UseCompression::No;
const contribution_is_compressed: UseCompression = UseCompression::Yes;
const compress_new_challenge: UseCompression = UseCompression::No;

fn main() {
    println!("Will verify and decompress a contribution to accumulator for 2^{} powers of tau", Bn256CeremonyParameters::REQUIRED_POWER);
    
    // Try to load `./challenge` from disk.
    let challenge_reader = OpenOptions::new()
                            .read(true)
                            .open("challenge").expect("unable open `./challenge` in this directory");

    {
        let metadata = challenge_reader.metadata().expect("unable to get filesystem metadata for `./challenge`");
        let expected_challenge_length = match previous_challenge_is_compressed {
            UseCompression::Yes => {
                Bn256CeremonyParameters::CONTRIBUTION_BYTE_SIZE
            },
            UseCompression::No => {
                Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE
            }
        };
        if metadata.len() != (expected_challenge_length as u64) {
            panic!("The size of `./challenge` should be {}, but it's {}, so something isn't right.", expected_challenge_length, metadata.len());
        }
    }

    let challenge_readable_map = unsafe { MmapOptions::new().map(&challenge_reader).expect("unable to create a memory map for input") };

    // Try to load `./response` from disk.
    let response_reader = OpenOptions::new()
                            .read(true)
                            .open("response").expect("unable open `./response` in this directory");

    {
        let metadata = response_reader.metadata().expect("unable to get filesystem metadata for `./response`");
        let expected_response_length = match contribution_is_compressed {
            UseCompression::Yes => {
                Bn256CeremonyParameters::CONTRIBUTION_BYTE_SIZE 
            },
            UseCompression::No => {
                Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE + Bn256CeremonyParameters::PUBLIC_KEY_SIZE
            }
        };
        if metadata.len() != (expected_response_length as u64) {
            panic!("The size of `./response` should be {}, but it's {}, so something isn't right.", expected_response_length, metadata.len());
        }
    }

    let response_readable_map = unsafe { MmapOptions::new().map(&response_reader).expect("unable to create a memory map for input") };

    println!("Calculating previous challenge hash...");

    // Check that contribution is correct

    let current_accumulator_hash = BachedAccumulator::<Bn256, Bn256CeremonyParameters>::calculate_hash(&challenge_readable_map);

    println!("Previous challenge hash");
    for line in current_accumulator_hash.as_slice().chunks(16) {
        print!("\t");
        for section in line.chunks(4) {
            for b in section {
                print!("{:02x}", b);
            }
            print!(" ");
        }
        println!("");
    }

    // Check the hash chain - a new response must be based on the previous challenge!
    {
        let mut response_challenge_hash = [0; 64];
        let memory_slice = response_readable_map.get(0..64).expect("must read point data from file");
        memory_slice.clone().read_exact(&mut response_challenge_hash).expect("couldn't read hash of challenge file from response file");

        println!("Response was based on the hash");
        for line in response_challenge_hash.chunks(16) {
            print!("\t");
            for section in line.chunks(4) {
                for b in section {
                    print!("{:02x}", b);
                }
                print!(" ");
            }
            println!("");
        }

        if &response_challenge_hash[..] != current_accumulator_hash.as_slice() {
            panic!("Hash chain failure. This is not the right response.");
        }
    }

    // get the contributor's public key
    let public_key = PublicKey::<Bn256>::read::<Bn256CeremonyParameters>(&response_readable_map, contribution_is_compressed)
                                           .expect("wasn't able to deserialize the response file's public key");


    // check that it follows the protocol

    let valid = BachedAccumulator::<Bn256, Bn256CeremonyParameters>::verify_transformation(
        &challenge_readable_map,
        &response_readable_map,
        &public_key, 
        current_accumulator_hash.as_slice(),
        previous_challenge_is_compressed,
        contribution_is_compressed,
        CheckForCorrectness::No,
        CheckForCorrectness::Yes,
    );

    if !valid {
        println!("Verification failed, contribution was invalid somehow.");
        panic!("INVALID CONTRIBUTION!!!");
    } else {
        println!("Verification succeeded!");
    }


    let response_hash = BachedAccumulator::<Bn256, Bn256CeremonyParameters>::calculate_hash(&response_readable_map);

    println!("Here's the BLAKE2b hash of the participant's response file:");

    for line in response_hash.as_slice().chunks(16) {
        print!("\t");
        for section in line.chunks(4) {
            for b in section {
                print!("{:02x}", b);
            }
            print!(" ");
        }
        println!("");
    }

    if compress_new_challenge == UseCompression::Yes {
        println!("Don't need to recompress the contribution, please copy `./response` as `./new_challenge`");
    } else {
        println!("Verification succeeded! Writing to `./new_challenge`...");

        // Create `./new_challenge` in this directory
        let writer = OpenOptions::new()
                                .read(true)
                                .write(true)
                                .create_new(true)
                                .open("new_challenge").expect("unable to create `./new_challenge` in this directory");



        // Recomputation stips the public key and uses hashing to link with the previous contibution after decompression
        writer.set_len(Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE as u64).expect("must make output file large enough");

        let mut writable_map = unsafe { MmapOptions::new().map_mut(&writer).expect("unable to create a memory map for output") };

        {
            (&mut writable_map[0..]).write(response_hash.as_slice()).expect("unable to write a default hash to mmap");

            writable_map.flush().expect("unable to write hash to `./new_challenge`");
        }

        BachedAccumulator::<Bn256, Bn256CeremonyParameters>::decompress(
            &response_readable_map,
            &mut writable_map,
            CheckForCorrectness::No).expect("must decompress a response for a new challenge");
        
        writable_map.flush().expect("must flush the memory map");

        println!("Done! `./new_challenge` contains the new challenge file. The other files");
        println!("were left alone.");
    }
}
