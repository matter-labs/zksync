extern crate powersoftau;
extern crate pairing;
use powersoftau::bn256::{Bn256CeremonyParameters};
use powersoftau::accumulator::{Accumulator};
use powersoftau::utils::{blank_hash};
use powersoftau::parameters::{UseCompression};

use std::fs::OpenOptions;
use std::io::{Write, BufWriter};
use pairing::bn256::Bn256;

fn main() {
    let writer = OpenOptions::new()
                            .read(false)
                            .write(true)
                            .create_new(true)
                            .open("challenge").expect("unable to create `./challenge`");

    let mut writer = BufWriter::new(writer);

    // Write a blank BLAKE2b hash:
    writer.write_all(&blank_hash().as_slice()).expect("unable to write blank hash to `./challenge`");

    let parameters = Bn256CeremonyParameters{};

    let acc: Accumulator<Bn256, _> = Accumulator::new(parameters);
    println!("Writing an empty accumulator to disk");
    acc.serialize(&mut writer, UseCompression::No).expect("unable to write fresh accumulator to `./challenge`");
    writer.flush().expect("unable to flush accumulator to disk");

    println!("Wrote a fresh accumulator to `./challenge`");
}
