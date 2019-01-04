extern crate powersoftau;
extern crate pairing;
extern crate memmap;

use powersoftau::bn256::{Bn256CeremonyParameters};
use powersoftau::batched_accumulator::{BachedAccumulator};
use powersoftau::utils::{blank_hash};

use std::fs::OpenOptions;
use std::io::{Write};
use pairing::bn256::Bn256;
use memmap::*;

use powersoftau::parameters::PowersOfTauParameters;

fn main() {
    let file = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create_new(true)
                            .open("challenge_constrained").expect("unable to create `./challenge`");

    let parameters = Bn256CeremonyParameters{};

    file.set_len(Bn256CeremonyParameters::ACCUMULATOR_BYTE_SIZE as u64).expect("unable to allocate large enough file");

    let mut writable_map = unsafe { MmapOptions::new().map_mut(&file).expect("unable to create a memory map") };

    // Write a blank BLAKE2b hash:
    let hash = blank_hash();
    (&mut writable_map[0..]).write(hash.as_slice()).expect("unable to write a default hash to mmap");

    writable_map.flush().expect("unable to write blank hash to `./challenge`");

    BachedAccumulator::<Bn256, _>::generate_initial(&mut writable_map, parameters).expect("generation of initial accumulator is successful");
    writable_map.flush().expect("unable to flush memmap to disk");

    println!("Wrote a fresh accumulator to `./challenge`");
}
