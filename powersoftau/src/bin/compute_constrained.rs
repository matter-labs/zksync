extern crate powersoftau;
extern crate pairing;
extern crate memmap;
extern crate rand;
extern crate blake2;
extern crate byteorder;

use powersoftau::bn256::{Bn256CeremonyParameters};
use powersoftau::batched_accumulator::{BachedAccumulator};
use powersoftau::keypair::{keypair};
use powersoftau::parameters::{UseCompression};

use std::fs::OpenOptions;
use pairing::bn256::Bn256;
use memmap::*;

use powersoftau::parameters::PowersOfTauParameters;

fn main() {
    // Create an RNG based on a mixture of system randomness and user provided randomness
    let mut rng = {
        use byteorder::{ReadBytesExt, BigEndian};
        use blake2::{Blake2b, Digest};
        use rand::{SeedableRng, Rng, OsRng};
        use rand::chacha::ChaChaRng;

        let h = {
            let mut system_rng = OsRng::new().unwrap();
            let mut h = Blake2b::default();

            // Gather 1024 bytes of entropy from the system
            for _ in 0..1024 {
                let r: u8 = system_rng.gen();
                h.input(&[r]);
            }

            // Ask the user to provide some information for additional entropy
            let mut user_input = String::new();
            println!("Type some random text and press [ENTER] to provide additional entropy...");
            std::io::stdin().read_line(&mut user_input).expect("expected to read some random text from the user");

            // Hash it all up to make a seed
            h.input(&user_input.as_bytes());
            h.result()
        };

        let mut digest = &h[..];

        // Interpret the first 32 bytes of the digest as 8 32-bit words
        let mut seed = [0u32; 8];
        for i in 0..8 {
            seed[i] = digest.read_u32::<BigEndian>().expect("digest is large enough for this to work");
        }

        ChaChaRng::from_seed(&seed)
    };

    let parameters = Bn256CeremonyParameters{};

    // Try to load `./challenge` from disk.
    let reader = OpenOptions::new()
                            .read(true)
                            .open("challenge_constrained").expect("unable open `./challenge` in this directory");

    {
        let metadata = reader.metadata().expect("unable to get filesystem metadata for `./challenge`");
        if metadata.len() != (Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE as u64) {
            panic!("The size of `./challenge` should be {}, but it's {}, so something isn't right.", Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE, metadata.len());
        }
    }

    let readable_map = unsafe { MmapOptions::new().map(&reader).expect("unable to create a memory map for input") };

    // Create `./response` in this directory
    let writer = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create_new(true)
                            .open("response_constrained").expect("unable to create `./response` in this directory");

    writer.set_len(Bn256CeremonyParameters::CONTRIBUTION_BYTE_SIZE as u64).expect("must make output file large enough");

    let mut writable_map = unsafe { MmapOptions::new().map_mut(&writer).expect("unable to create a memory map for output") };
    
    println!("Calculating previous contribution hash...");

    let current_accumulator_hash = BachedAccumulator::<Bn256, Bn256CeremonyParameters>::calculate_hash(&readable_map);

    // Construct our keypair using the RNG we created above
    let (pubkey, privkey) = keypair(&mut rng, current_accumulator_hash.as_ref());

    // Perform the transformation
    println!("Computing and writing your contribution, this could take a while...");

    // this computes a transformation and writes it in compressed form
    BachedAccumulator::<Bn256, Bn256CeremonyParameters>::transform(&readable_map, &mut writable_map, parameters.clone(), &privkey).expect("must transform the key");

    println!("Finihsing writing your contribution to `./response`...");

    // Write the public key
    pubkey.write::<Bn256CeremonyParameters>(&mut writable_map).expect("unable to write public key");

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

    // check transformation

    let valid = BachedAccumulator::<Bn256, Bn256CeremonyParameters>::verify_transformation(
        &readable_map,
        &output_readonly,
        parameters.clone(),
        &pubkey,
        &contribution_hash.as_slice(),
        UseCompression::Yes,
    );

    if !valid {
        panic!("Invalid contribution upon verification");
    }

    println!("\n");
}
