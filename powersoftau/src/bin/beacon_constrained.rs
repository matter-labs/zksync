extern crate powersoftau;
extern crate pairing;
extern crate memmap;
extern crate rand;
extern crate blake2;
extern crate byteorder;
extern crate crypto;

// use powersoftau::bn256::{Bn256CeremonyParameters};
use powersoftau::small_bn256::{Bn256CeremonyParameters};
use powersoftau::batched_accumulator::{BachedAccumulator};
use powersoftau::keypair::{keypair};
use powersoftau::parameters::{UseCompression, CheckForCorrectness};

use std::fs::OpenOptions;
use pairing::bn256::Bn256;
use memmap::*;

use std::io::Write;

use powersoftau::parameters::PowersOfTauParameters;

#[macro_use]
extern crate hex_literal;

const input_is_compressed: UseCompression = UseCompression::No;
const compress_the_output: UseCompression = UseCompression::Yes;
const check_input_correctness: CheckForCorrectness = CheckForCorrectness::No;


fn main() {
    println!("Will contribute a random beacon to accumulator for 2^{} powers of tau", Bn256CeremonyParameters::REQUIRED_POWER);
    println!("In total will generate up to {} powers", Bn256CeremonyParameters::TAU_POWERS_G1_LENGTH);
    
    // Create an RNG based on the outcome of the random beacon
    let mut rng = {
        use byteorder::{ReadBytesExt, BigEndian};
        use rand::{SeedableRng};
        use rand::chacha::ChaChaRng;
        use crypto::sha2::Sha256;
        use crypto::digest::Digest;

        // Place block hash here (block number #514200)
        let mut cur_hash: [u8; 32] = hex!("00000000000000000034b33e842ac1c50456abe5fa92b60f6b3dfc5d247f7b58");

        // Performs 2^n hash iterations over it
        // const N: usize = 42;

        const N: usize = 16;

        for i in 0..(1u64<<N) {
            // Print 1024 of the interstitial states
            // so that verification can be
            // parallelized

            // if i % (1u64<<(N-10)) == 0 {
            //     print!("{}: ", i);
            //     for b in cur_hash.iter() {
            //         print!("{:02x}", b);
            //     }
            //     println!("");
            // }

            let mut h = Sha256::new();
            h.input(&cur_hash);
            h.result(&mut cur_hash);
        }

        print!("Final result of beacon: ");
        for b in cur_hash.iter() {
            print!("{:02x}", b);
        }
        println!("");

        let mut digest = &cur_hash[..];

        let mut seed = [0u32; 8];
        for i in 0..8 {
            seed[i] = digest.read_u32::<BigEndian>().expect("digest is large enough for this to work");
        }

        ChaChaRng::from_seed(&seed)
    };

    println!("Done creating a beacon RNG");

    // Try to load `./challenge` from disk.
    let reader = OpenOptions::new()
                            .read(true)
                            .open("challenge").expect("unable open `./challenge` in this directory");

    {
        let metadata = reader.metadata().expect("unable to get filesystem metadata for `./challenge`");
        let expected_challenge_length = match input_is_compressed {
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

    let readable_map = unsafe { MmapOptions::new().map(&reader).expect("unable to create a memory map for input") };

    // Create `./response` in this directory
    let writer = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create_new(true)
                            .open("response").expect("unable to create `./response` in this directory");

    let required_output_length = match compress_the_output {
        UseCompression::Yes => {
            Bn256CeremonyParameters::CONTRIBUTION_BYTE_SIZE
        },
        UseCompression::No => {
            Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE + Bn256CeremonyParameters::PUBLIC_KEY_SIZE
        }
    };

    writer.set_len(required_output_length as u64).expect("must make output file large enough");

    let mut writable_map = unsafe { MmapOptions::new().map_mut(&writer).expect("unable to create a memory map for output") };
    
    println!("Calculating previous contribution hash...");

    let current_accumulator_hash = BachedAccumulator::<Bn256, Bn256CeremonyParameters>::calculate_hash(&readable_map);

    {
        println!("Contributing on top of the hash:");
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

        (&mut writable_map[0..]).write(current_accumulator_hash.as_slice()).expect("unable to write a challenge hash to mmap");

        writable_map.flush().expect("unable to write hash to `./response`");
    }

    // Construct our keypair using the RNG we created above
    let (pubkey, privkey) = keypair(&mut rng, current_accumulator_hash.as_ref());

    // Perform the transformation
    println!("Computing and writing your contribution, this could take a while...");

    // this computes a transformation and writes it
    BachedAccumulator::<Bn256, Bn256CeremonyParameters>::transform(
        &readable_map, 
        &mut writable_map, 
        input_is_compressed, 
        compress_the_output, 
        check_input_correctness, 
        &privkey
    ).expect("must transform with the key");
    println!("Finihsing writing your contribution to `./response`...");

    // Write the public key
    pubkey.write::<Bn256CeremonyParameters>(&mut writable_map, compress_the_output).expect("unable to write public key");

    // Get the hash of the contribution, so the user can compare later
    let output_readonly = writable_map.make_read_only().expect("must make a map readonly");
    let contribution_hash = BachedAccumulator::<Bn256, Bn256CeremonyParameters>::calculate_hash(&output_readonly);

    print!("Done!\n\n\
              Your contribution has been written to `./response`\n\n\
              The BLAKE2b hash of `./response` is:\n");

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

    println!("Thank you for your participation, much appreciated! :)");
}
