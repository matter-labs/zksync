// #![deny(warnings, rust_2018_idioms)]
// #![feature(custom_inner_attributes)]
#![feature(target_feature)]
#![feature(stdsimd)]
#![feature(avx512_target_feature)]

extern crate num_bigint;
extern crate num_integer;
extern crate num_traits;
extern crate rand;

use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{One, ToPrimitive, Zero};
use std::str::FromStr;


pub struct U256 (pub [u64; 4]);

// impl ::ff::PrimeFieldRepr for U256 {
//     #[inline(always)]
//     fn is_odd(&self) -> bool {
//         self.0[0] & 1 == 1
//     }

//     #[inline(always)]
//     fn is_even(&self) -> bool {
//         !self.is_odd()
//     }

//     #[inline(always)]
//     fn is_zero(&self) -> bool {
//         self.0.iter().all(|&e| e == 0)
//     }

//     #[inline(always)]
//     fn shr(&mut self, mut n: u32) {
//         if n as usize >= 256 {
//             *self = Self::from(0);
//             return;
//         }

//         while n >= 64 {
//             let mut t = 0;
//             for i in self.0.iter_mut().rev() {
//                 ::std::mem::swap(&mut t, i);
//             }
//             n -= 64;
//         }

//         if n > 0 {
//             let mut t = 0;
//             for i in self.0.iter_mut().rev() {
//                 let t2 = *i << (64 - n);
//                 *i >>= n;
//                 *i |= t;
//                 t = t2;
//             }
//         }
//     }

//     #[inline(always)]
//     fn div2(&mut self) {
//         let mut t = 0;
//         for i in self.0.iter_mut().rev() {
//             let t2 = *i << 63;
//             *i >>= 1;
//             *i |= t;
//             t = t2;
//         }
//     }

//     #[inline(always)]
//     fn mul2(&mut self) {
//         let mut last = 0;
//         for i in &mut self.0 {
//             let tmp = *i >> 63;
//             *i <<= 1;
//             *i |= last;
//             last = tmp;
//         }
//     }

//     #[inline(always)]
//     fn shl(&mut self, mut n: u32) {
//         if n as usize >= 256 {
//             *self = Self::from(0);
//             return;
//         }

//         while n >= 64 {
//             let mut t = 0;
//             for i in &mut self.0 {
//                 ::std::mem::swap(&mut t, i);
//             }
//             n -= 64;
//         }

//         if n > 0 {
//             let mut t = 0;
//             for i in &mut self.0 {
//                 let t2 = *i >> (64 - n);
//                 *i <<= n;
//                 *i |= t;
//                 t = t2;
//             }
//         }
//     }

//     #[inline(always)]
//     fn num_bits(&self) -> u32 {
//         let mut ret = 256;
//         for i in self.0.iter().rev() {
//             let leading = i.leading_zeros();
//             ret -= leading;
//             if leading != 64 {
//                 break;
//             }
//         }

//         ret
//     }

//     #[inline(always)]
//     fn add_nocarry(&mut self, other: &Self) {
//         let mut carry = 0;

//         for (a, b) in self.0.iter_mut().zip(other.0.iter()) {
//             *a = ::ff::adc(*a, *b, &mut carry);
//         }
//     }

//     #[inline(always)]
//     fn sub_noborrow(&mut self, other: &Self) {
//         let mut borrow = 0;

//         for (a, b) in self.0.iter_mut().zip(other.0.iter()) {
//             *a = ::ff::sbb(*a, *b, &mut borrow);
//         }
//     }
// }

// /// Convert BigUint into a vector of 64-bit limbs.
// fn biguint_to_real_u64_vec(mut v: BigUint, limbs: usize) -> Vec<u64> {
//     let m = BigUint::one() << 64;
//     let mut ret = vec![];

//     while v > BigUint::zero() {
//         ret.push((&v % &m).to_u64().unwrap());
//         v = v >> 64;
//     }

//     while ret.len() < limbs {
//         ret.push(0);
//     }

//     assert!(ret.len() == limbs);

//     ret
// }

// fn biguint_num_bits(mut v: BigUint) -> u32 {
//     let mut bits = 0;

//     while v != BigUint::zero() {
//         v = v >> 1;
//         bits += 1;
//     }

//     bits
// }

// /// BigUint modular exponentiation by square-and-multiply.
// fn exp(base: BigUint, exp: &BigUint, modulus: &BigUint) -> BigUint {
//     let mut ret = BigUint::one();

//     for i in exp.to_bytes_be()
//         .into_iter()
//         .flat_map(|x| (0..8).rev().map(move |i| (x >> i).is_odd()))
//     {
//         ret = (&ret * &ret) % modulus;
//         if i {
//             ret = (ret * &base) % modulus;
//         }
//     }

//     ret
// }

mod arith_impl_asm {
    extern crate stdsimd;
    extern crate packed_simd;
    use std::{u64, mem::transmute};
    use std::arch::x86_64::*;
    use self::stdsimd::*;

    // const BROADCAST_MASK: [__m256i; 16] = get_broadcast_mask();


    unsafe fn get_broadcast_mask() -> [__m256i; 16] 
    {
        [

            _mm256_set_epi64x(0x8000000000000000, 0x8000000000000000, 0x8000000000000000, 0x8000000000000000),

            _mm256_set_epi64x(0x8000000000000000, 0x8000000000000000, 0x8000000000000000, 0x8000000000000001),

            _mm256_set_epi64x(0x8000000000000000, 0x8000000000000000, 0x8000000000000001, 0x8000000000000000),

            _mm256_set_epi64x(0x8000000000000000, 0x8000000000000000, 0x8000000000000001, 0x8000000000000001),

            _mm256_set_epi64x(0x8000000000000000, 0x8000000000000001, 0x8000000000000000, 0x8000000000000000),

            _mm256_set_epi64x(0x8000000000000000, 0x8000000000000001, 0x8000000000000000, 0x8000000000000001),

            _mm256_set_epi64x(0x8000000000000000, 0x8000000000000001, 0x8000000000000001, 0x8000000000000000),

            _mm256_set_epi64x(0x8000000000000000, 0x8000000000000001, 0x8000000000000001, 0x8000000000000001),

            _mm256_set_epi64x(0x8000000000000001, 0x8000000000000000, 0x8000000000000000, 0x8000000000000000),

            _mm256_set_epi64x(0x8000000000000001, 0x8000000000000000, 0x8000000000000000, 0x8000000000000001),

            _mm256_set_epi64x(0x8000000000000001, 0x8000000000000000, 0x8000000000000001, 0x8000000000000000),

            _mm256_set_epi64x(0x8000000000000001, 0x8000000000000000, 0x8000000000000001, 0x8000000000000001),

            _mm256_set_epi64x(0x8000000000000001, 0x8000000000000001, 0x8000000000000000, 0x8000000000000000),

            _mm256_set_epi64x(0x8000000000000001, 0x8000000000000001, 0x8000000000000000, 0x8000000000000001),

            _mm256_set_epi64x(0x8000000000000001, 0x8000000000000001, 0x8000000000000001, 0x8000000000000000),

            _mm256_set_epi64x(0x8000000000000001, 0x8000000000000001, 0x8000000000000001, 0x8000000000000001),

            ]
    }

    // #[inline]
    // #[allow(unsafe_code)]
    // #[target_feature(enable = "avx512f")]
    // #[target_feature(enable = "fma")]
    // #[cfg(target_arch = "x86_64")]
    // // #[inline(always)]
    // pub unsafe fn full_addition_avx512(a: [u64; 4], b: [u64; 4]) -> [u64; 4] {

    //     let MAX_WORD = _mm512_set1_epi64(0xffffffffffffffff);
    //     let mut a = _mm512_set_epi64x(transmute::<u64, i64>(a[0]),
    //                                     transmute::<u64, i64>(a[1]),
    //                                     transmute::<u64, i64>(a[2]),
    //                                     transmute::<u64, i64>(a[3]),
    //                                     0,0,0,0);
    //     // a = _mm256_xor_si256(a, _mm256_set1_epi64x(0x8000000000000000));

    //     let b = _mm512_set_epi64x(transmute::<u64, i64>(b[0]),
    //                                     transmute::<u64, i64>(b[1]),
    //                                     transmute::<u64, i64>(b[2]),
    //                                     transmute::<u64, i64>(b[3]),
    //                                     0,0,0,0);


 

    //     let s = _mm512_add_epi64(a, b);

    //     let c = _mm512_cmplt_epu64_mask(s, a);

    //     let mut m = _mm512_cmpeq_epi64_mask(s, MAX_WORD);

 

    //     {

    //         let c0 = _mm512_mask2int(c);

    //         let mut m0 = _mm512_mask2int(m);

    //         let mut carry = m0;

    //         carry = (carry + c0*2); //  lea

    //         m0 ^= carry;

    //         // carry >>= 8;

    //         m = _mm512_int2mask(m0);

    //     }

 

    //     return _mm512_mask_sub_epi64(s, m, s, MAX_WORD);

    // }

    #[inline]
    #[allow(unsafe_code)]
    #[target_feature(enable = "avx512f")]
    #[target_feature(enable = "fma")]
    #[cfg(target_arch = "x86_64")]
    // #[inline(always)]
    pub unsafe fn full_addition(a: [u64; 4], b: [u64; 4]) -> [u64; 4] {
        let mask = get_broadcast_mask();
        let mut a = _mm256_set_epi64x(transmute::<u64, i64>(a[0]),
                                        transmute::<u64, i64>(a[1]),
                                        transmute::<u64, i64>(a[2]),
                                        transmute::<u64, i64>(a[3]));
        a = _mm256_xor_si256(a, _mm256_set1_epi64x(0x8000000000000000));

        let b = _mm256_set_epi64x(transmute::<u64, i64>(b[0]),
                                        transmute::<u64, i64>(b[1]),
                                        transmute::<u64, i64>(b[2]),
                                        transmute::<u64, i64>(b[3]));

        let s = _mm256_add_epi64(a, b);

        let cv = _mm256_cmpgt_epi64(a, s);

        let mv = _mm256_cmpeq_epi64(s, _mm256_set1_epi64x(0x7fffffffffffffff));

        let mut c: i32 = _mm256_movemask_pd(_mm256_castsi256_pd(cv));

        let mut m: i32 = _mm256_movemask_pd(_mm256_castsi256_pd(mv));

        {

            c = m + 2*c; //  lea

            m ^= c;

            m &= 0x0f;

        }

        let res = _mm256_add_epi64(s, mask[m as usize]);


        let mask = _mm256_set_epi64x(transmute::<u64, i64>(0x8000000000000000),
                                        transmute::<u64, i64>(0x8000000000000000),
                                        transmute::<u64, i64>(0x8000000000000000),
                                        transmute::<u64, i64>(0x8000000000000000));

        let mut answer = [0u64; 4];

        _mm256_maskstore_epi64(answer[..].as_mut_ptr() as *mut i64, mask ,res); 

        [answer[3], answer[2], answer[1], answer[0]]
    }

    #[inline]
    #[allow(unsafe_code)]
    #[target_feature(enable = "avx2")]
    #[target_feature(enable = "fma")]
    #[cfg(target_arch = "x86_64")]
    // #[inline(always)]
    pub unsafe fn cos_mul(a: [u64; 4], b: [u64; 4]) -> [u64; 8] {
        let mut r_ac = [0u128; 9];
        let mut r_low = [0u128; 9];
        let mut r_high = [0u128; 9];
        let rb_0_rb2 = _mm256_set_epi64x(transmute::<u64, i64>(b[0]),
                                        transmute::<u64, i64>(0),
                                        transmute::<u64, i64>(b[2]),
                                        transmute::<u64, i64>(0));
        let rb_1_rb3 = _mm256_set_epi64x(transmute::<u64, i64>(b[1]),
                                        transmute::<u64, i64>(0),
                                        transmute::<u64, i64>(b[3]),
                                        transmute::<u64, i64>(0));

        let a0_a0 = _mm256_set_epi64x(transmute::<u64, i64>(a[0]),
                                        transmute::<u64, i64>(0),
                                        transmute::<u64, i64>(a[0]),
                                        transmute::<u64, i64>(0));

        let a1_a1 = _mm256_set_epi64x(transmute::<u64, i64>(a[1]),
                                        transmute::<u64, i64>(0),
                                        transmute::<u64, i64>(a[1]),
                                        transmute::<u64, i64>(0));

        let a2_a2 = _mm256_set_epi64x(transmute::<u64, i64>(a[2]),
                                        transmute::<u64, i64>(0),
                                        transmute::<u64, i64>(a[2]),
                                        transmute::<u64, i64>(0));

        let a3_a3 = _mm256_set_epi64x(transmute::<u64, i64>(a[3]),
                                        transmute::<u64, i64>(0),
                                        transmute::<u64, i64>(a[3]),
                                        transmute::<u64, i64>(0));

        let r_ac_0_2_reg = _mm256_set_epi64x(0,0,0,0);
        let r_ac_1_3_reg = _mm256_set_epi64x(0,0,0,0);
        let tmp = _mm256__
        // let r_ac_2_4_reg = _mm256_set_epi64x(0,0,0,0);

        for i in 0..4 {

        }
        let mut a = _mm256_set_epi64x(transmute::<u64, i64>(a[0]),
                                        transmute::<u64, i64>(a[1]),
                                        transmute::<u64, i64>(a[2]),
                                        transmute::<u64, i64>(a[3]));
        a = _mm256_xor_si256(a, _mm256_set1_epi64x(0x8000000000000000));

        let b = _mm256_set_epi64x(transmute::<u64, i64>(b[0]),
                                        transmute::<u64, i64>(b[1]),
                                        transmute::<u64, i64>(b[2]),
                                        transmute::<u64, i64>(b[3]));

        let s = _mm256_add_epi64(a, b);

        let cv = _mm256_cmpgt_epi64(a, s);

        let mv = _mm256_cmpeq_epi64(s, _mm256_set1_epi64x(0x7fffffffffffffff));

        let mut c: i32 = _mm256_movemask_pd(_mm256_castsi256_pd(cv));

        let mut m: i32 = _mm256_movemask_pd(_mm256_castsi256_pd(mv));

        {

            c = m + 2*c; //  lea

            m ^= c;

            m &= 0x0f;

        }

        let res = _mm256_add_epi64(s, mask[m as usize]);


        let mask = _mm256_set_epi64x(transmute::<u64, i64>(0x8000000000000000),
                                        transmute::<u64, i64>(0x8000000000000000),
                                        transmute::<u64, i64>(0x8000000000000000),
                                        transmute::<u64, i64>(0x8000000000000000));

        let mut answer = [0u64; 4];

        _mm256_maskstore_epi64(answer[..].as_mut_ptr() as *mut i64, mask ,res); 

        [answer[3], answer[2], answer[1], answer[0]]
    }

    pub fn add_nocarry(a: [u64; 4], b: [u64; 4]) -> [u64; 4] {
                let mut carry = 0;
                let mut c = [0u64; 4];

                for (i, (a, b)) in a.iter().zip(b.iter()).enumerate() {
                    c[i] = adc(*a, *b, &mut carry);
                }
                
                c
            }

    /// Calculate a + b + carry, returning the sum and modifying the
    /// carry value.
    #[inline(always)]
    pub fn adc(a: u64, b: u64, carry: &mut u64) -> u64 {
        let tmp = u128::from(a) + u128::from(b) + u128::from(*carry);

        *carry = (tmp >> 64) as u64;

        tmp as u64
    }

    /// Calculate a + (b * c) + carry, returning the least significant digit
    /// and setting carry to the most significant digit.
    #[inline(always)]
    pub fn mac_with_carry(a: u64, b: u64, c: u64, carry: &mut u64) -> u64 {
        let tmp = (u128::from(a)) + u128::from(b) * u128::from(c) + u128::from(*carry);

        *carry = (tmp >> 64) as u64;

        tmp as u64
    }
}

#[test]
fn test_full_add() {
    let a = [1, 2, 3, 4];
    let b = [5, 6, 7, 8];
    let c = unsafe {
        arith_impl_asm::full_addition(a, b)
    };
    for num in &c {
        println!("{}", num);
    }
}

#[test]
fn test_trivial_add() {
    let a = [1, 2, 3, 4];
    let b = [5, 6, 7, 8];
    let c = arith_impl_asm::add_nocarry(a, b);
    for num in &c {
        println!("{}", num);
    }
}

#[test]
fn test_additions() {
    use rand::{SeedableRng, Rng, XorShiftRng};
    let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    for _ in 0..100 {
        let a = [rng.gen(), rng.gen(), rng.gen(), rng.gen()];
        let b = [rng.gen(), rng.gen(), rng.gen(), rng.gen()];
        let c = arith_impl_asm::add_nocarry(a, b);
        let d = unsafe {
            arith_impl_asm::full_addition(a, b)
        };
        assert_eq!(c, d);

    }
}

