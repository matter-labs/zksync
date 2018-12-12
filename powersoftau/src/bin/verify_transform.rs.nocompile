extern crate powersoftau;
use powersoftau::*;

use std::fs::OpenOptions;
use std::io::{Read, Write, BufWriter, BufReader};

fn main() {
    // Try to load `./challenge` from disk.
    let challenge_reader = OpenOptions::new()
                            .read(true)
                            .open("challenge").expect("unable open `./challenge` in this directory");

    {
        let metadata = challenge_reader.metadata().expect("unable to get filesystem metadata for `./challenge`");
        if metadata.len() != (ACCUMULATOR_BYTE_SIZE as u64) {
            panic!("The size of `./challenge` should be {}, but it's {}, so something isn't right.", ACCUMULATOR_BYTE_SIZE, metadata.len());
        }
    }

    let challenge_reader = BufReader::new(challenge_reader);
    let mut challenge_reader = HashReader::new(challenge_reader);

    // Try to load `./response` from disk.
    let response_reader = OpenOptions::new()
                            .read(true)
                            .open("response").expect("unable open `./response` in this directory");

    {
        let metadata = response_reader.metadata().expect("unable to get filesystem metadata for `./response`");
        if metadata.len() != (CONTRIBUTION_BYTE_SIZE as u64) {
            panic!("The size of `./response` should be {}, but it's {}, so something isn't right.", CONTRIBUTION_BYTE_SIZE, metadata.len());
        }
    }

    let response_reader = BufReader::new(response_reader);
    let mut response_reader = HashReader::new(response_reader);

    // Create new_challenge file
    let writer = OpenOptions::new()
                            .read(false)
                            .write(true)
                            .create_new(true)
                            .open("new_challenge").expect("unable to create `./new_challenge`");

    let mut writer = BufWriter::new(writer);

    // Deserialize the current challenge

    // Read the BLAKE2b hash of the previous contribution
    {
        // We don't need to do anything with it, but it's important for
        // the hash chain.
        let mut tmp = [0; 64];
        challenge_reader.read_exact(&mut tmp).expect("unable to read BLAKE2b hash of previous contribution");
    }

    // Load the current accumulator into memory
    let current_accumulator = Accumulator::deserialize(
        &mut challenge_reader,
        UseCompression::No,
        CheckForCorrectness::No // no need to check since we constructed the challenge already
    ).expect("unable to read uncompressed accumulator");

    // Get the hash of the current accumulator
    let current_accumulator_hash = challenge_reader.into_hash();

    // Load the response into memory

    // Check the hash chain
    {
        let mut response_challenge_hash = [0; 64];
        response_reader.read_exact(&mut response_challenge_hash).expect("couldn't read hash of challenge file from response file");

        if &response_challenge_hash[..] != current_accumulator_hash.as_slice() {
            panic!("Hash chain failure. This is not the right response.");
        }
    }

    // Load the response's accumulator
    let new_accumulator = Accumulator::deserialize(&mut response_reader, UseCompression::Yes, CheckForCorrectness::Yes)
                                                  .expect("wasn't able to deserialize the response file's accumulator");

    // Load the response's pubkey
    let public_key = PublicKey::deserialize(&mut response_reader)
                                           .expect("wasn't able to deserialize the response file's public key");

    // Get the hash of the response file
    let response_hash = response_reader.into_hash();

    if !verify_transform(&current_accumulator, &new_accumulator, &public_key, current_accumulator_hash.as_slice()) {
        println!("Verification failed, contribution was invalid somehow.");
        panic!("INVALID CONTRIBUTION!!!");
    } else {
        println!("Verification succeeded!");
    }

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

    println!("Verification succeeded! Writing to `./new_challenge`...");

    writer.write_all(response_hash.as_slice()).expect("couldn't write response file's hash into the `./new_challenge` file");
    new_accumulator.serialize(&mut writer, UseCompression::No).expect("unable to write uncompressed accumulator into the `./new_challenge` file");

    println!("Done! `./new_challenge` contains the new challenge file. The other files");
    println!("were left alone.");
}
