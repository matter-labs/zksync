extern crate powersoftau;
use powersoftau::*;

use std::fs::OpenOptions;
use std::io::{Write, BufWriter};

fn main() {
    let writer = OpenOptions::new()
                            .read(false)
                            .write(true)
                            .create_new(true)
                            .open("challenge").expect("unable to create `./challenge`");

    let mut writer = BufWriter::new(writer);

    // Write a blank BLAKE2b hash:
    writer.write_all(&blank_hash().as_slice()).expect("unable to write blank hash to `./challenge`");

    let acc = Accumulator::new();
    acc.serialize(&mut writer, UseCompression::No).expect("unable to write fresh accumulator to `./challenge`");
    writer.flush().expect("unable to flush accumulator to disk");

    println!("Wrote a fresh accumulator to `./challenge`");
}
