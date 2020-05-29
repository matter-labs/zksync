// External uses
use criterion::{black_box, criterion_group, BatchSize, Bencher, Criterion, Throughput};
// Local uses
use models::primitives::{
    bytes_into_be_bits, get_bits_le_fixed_u128, pack_bits_into_bytes,
    pack_bits_into_bytes_in_order, BitIteratorLe, GetBits,
};

/// Input size for byte slices (module-wide for calculating the throughput).
const BYTE_SLICE_SIZE: usize = 512;

fn bench_u64_get_bits_le(b: &mut Bencher<'_>) {
    let value: u64 = 0xDEAD_BEEF_DEAD_BEEF;

    b.iter(|| {
        let _ = black_box(value).get_bits_le();
    });
}

fn bench_get_bits_le_fixed_u128(b: &mut Bencher<'_>) {
    let value: u128 = 0xDEAD_BEEF_DEAD_BEEF_DEAD_BEEF_DEAD_BEEF;
    let n = 128;

    b.iter(|| {
        let _ = get_bits_le_fixed_u128(black_box(value), n);
    });
}

fn bench_bytes_into_be_bits(b: &mut Bencher<'_>) {
    let value: Vec<u8> = vec![0xAB; BYTE_SLICE_SIZE];

    let value_ref: &[u8] = value.as_ref();

    b.iter(|| {
        let _ = bytes_into_be_bits(black_box(value_ref));
    });
}

fn bench_pack_bits_into_bytes(b: &mut Bencher<'_>) {
    let value: Vec<bool> = vec![true; BYTE_SLICE_SIZE * 8];

    let setup = || value.clone();

    b.iter_batched(
        setup,
        |value| {
            let _ = pack_bits_into_bytes(black_box(value));
        },
        BatchSize::SmallInput,
    );
}

fn bench_pack_bits_into_bytes_in_order(b: &mut Bencher<'_>) {
    let value: Vec<bool> = vec![true; BYTE_SLICE_SIZE * 8];

    let setup = || value.clone();

    b.iter_batched(
        setup,
        |value| {
            let _ = pack_bits_into_bytes_in_order(black_box(value));
        },
        BatchSize::SmallInput,
    );
}

fn bench_bit_iterator_le_next(b: &mut Bencher<'_>) {
    let value: Vec<u64> = vec![0xDEAD_BEEF_DEAD_BEEF; BYTE_SLICE_SIZE / 8];

    let setup = || BitIteratorLe::new(&value);

    b.iter_batched(
        setup,
        |bit_iterator| {
            for _ in bit_iterator {
                // Do nothing, we're just draining the iterator.
            }
        },
        BatchSize::SmallInput,
    );
}

pub fn bench_primitives(c: &mut Criterion) {
    c.bench_function("u64_get_bits_le", bench_u64_get_bits_le);
    c.bench_function("get_bits_le_fixed_u128", bench_get_bits_le_fixed_u128);

    let mut group = c.benchmark_group("Bit Converters");

    group.throughput(Throughput::Bytes(BYTE_SLICE_SIZE as u64));
    group.bench_function("bytes_into_be_bits", bench_bytes_into_be_bits);
    group.bench_function("pack_bits_into_bytes", bench_pack_bits_into_bytes);
    group.bench_function(
        "pack_bits_into_bytes_in_order",
        bench_pack_bits_into_bytes_in_order,
    );
    group.bench_function("BitIterator::next", bench_bit_iterator_le_next);

    group.finish();
}

criterion_group!(primitives_benches, bench_primitives);
