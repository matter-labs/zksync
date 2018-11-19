use super::fq::{FROBENIUS_COEFF_FQ2_C1, Fq, NEGATIVE_ONE};
use ff::{Field, SqrtField};
use rand::{Rand, Rng};

use std::cmp::Ordering;

/// An element of Fq2, represented by c0 + c1 * u.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Fq2 {
    pub c0: Fq,
    pub c1: Fq,
}

impl ::std::fmt::Display for Fq2 {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "Fq2({} + {} * u)", self.c0, self.c1)
    }
}

/// `Fq2` elements are ordered lexicographically.
impl Ord for Fq2 {
    #[inline(always)]
    fn cmp(&self, other: &Fq2) -> Ordering {
        match self.c1.cmp(&other.c1) {
            Ordering::Greater => Ordering::Greater,
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.c0.cmp(&other.c0),
        }
    }
}

impl PartialOrd for Fq2 {
    #[inline(always)]
    fn partial_cmp(&self, other: &Fq2) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Fq2 {
    // This is very confusing part, cause depends not of Fq2 itself, but form Fq6 construction

    /// Multiply this element by quadratic nonresidue 9 + u.
    pub fn mul_by_nonresidue(&mut self) {
        // (xi+y)(i+9) = (9x+y)i+(9y-x)
        let t0 = self.c0;
        let t1 = self.c1;

        // 8*x*i + 8*y
        self.double();
        self.double();
        self.double();

        // 9*y
        self.c0.add_assign(&t0);
        // (9*y - x)
        self.c0.sub_assign(&t1);

        // (9*x)i
        self.c1.add_assign(&t1);
        // (9*x + y)
        self.c1.add_assign(&t0);
    }

    // Multiply this element by ξ where ξ=i+9
    pub fn mul_by_xi(&mut self) {
        // (xi+y)(i+9) = (9x+y)i+(9y-x)
        let t0 = self.c0;
        let t1 = self.c1;

        // 8*x*i + 8*y
        self.double();
        self.double();
        self.double();

        // 9*y
        self.c0.add_assign(&t0);
        // (9*y - x)
        self.c0.sub_assign(&t1);

        // (9*x)i
        self.c1.add_assign(&t1);
        // (9*x + y)
        self.c1.add_assign(&t0);
    }

    /// Norm of Fq2 as extension field in i over Fq
    pub fn norm(&self) -> Fq {
        let mut t0 = self.c0;
        let mut t1 = self.c1;
        t0.square();
        t1.square();
        t1.add_assign(&t0);

        t1
    }

    // conjucate by negating c1
    pub fn conjugate(&mut self) {
        self.c1.negate();
    }
}

impl Rand for Fq2 {
    fn rand<R: Rng>(rng: &mut R) -> Self {
        Fq2 {
            c0: rng.gen(),
            c1: rng.gen(),
        }
    }
}

impl Field for Fq2 {
    fn zero() -> Self {
        Fq2 {
            c0: Fq::zero(),
            c1: Fq::zero(),
        }
    }

    fn one() -> Self {
        Fq2 {
            c0: Fq::one(),
            c1: Fq::zero(),
        }
    }

    fn is_zero(&self) -> bool {
        self.c0.is_zero() && self.c1.is_zero()
    }

    fn square(&mut self) {
        let mut ab = self.c0;
        ab.mul_assign(&self.c1);
        let mut c0c1 = self.c0;
        c0c1.add_assign(&self.c1);
        let mut c0 = self.c1;
        c0.negate();
        c0.add_assign(&self.c0);
        c0.mul_assign(&c0c1);
        c0.sub_assign(&ab);
        self.c1 = ab;
        self.c1.add_assign(&ab);
        c0.add_assign(&ab);
        self.c0 = c0;
    }

    fn double(&mut self) {
        self.c0.double();
        self.c1.double();
    }

    fn negate(&mut self) {
        self.c0.negate();
        self.c1.negate();
    }

    fn add_assign(&mut self, other: &Self) {
        self.c0.add_assign(&other.c0);
        self.c1.add_assign(&other.c1);
    }

    fn sub_assign(&mut self, other: &Self) {
        self.c0.sub_assign(&other.c0);
        self.c1.sub_assign(&other.c1);
    }

    fn mul_assign(&mut self, other: &Self) {
        let mut aa = self.c0;
        aa.mul_assign(&other.c0);
        let mut bb = self.c1;
        bb.mul_assign(&other.c1);
        let mut o = other.c0;
        o.add_assign(&other.c1);
        self.c1.add_assign(&self.c0);
        self.c1.mul_assign(&o);
        self.c1.sub_assign(&aa);
        self.c1.sub_assign(&bb);
        self.c0 = aa;
        self.c0.sub_assign(&bb);
    }

    fn inverse(&self) -> Option<Self> {
        let mut t1 = self.c1;
        t1.square();
        let mut t0 = self.c0;
        t0.square();
        t0.add_assign(&t1);
        t0.inverse().map(|t| {
            let mut tmp = Fq2 {
                c0: self.c0,
                c1: self.c1,
            };
            tmp.c0.mul_assign(&t);
            tmp.c1.mul_assign(&t);
            tmp.c1.negate();

            tmp
        })
    }

    fn frobenius_map(&mut self, power: usize) {
        self.c1.mul_assign(&FROBENIUS_COEFF_FQ2_C1[power % 2]);
    }
}

impl SqrtField for Fq2 {
    fn legendre(&self) -> ::ff::LegendreSymbol {
        self.norm().legendre()
    }

    fn sqrt(&self) -> Option<Self> {
        // Algorithm 9, https://eprint.iacr.org/2012/685.pdf

        if self.is_zero() {
            Some(Self::zero())
        } else {
            // a1 = self^((q - 3) / 4)
            let mut a1 = self.pow([
                0x4f082305b61f3f51,
                0x65e05aa45a1c72a3,
                0x6e14116da0605617,
                0x0c19139cb84c680a,
            ]);
            let mut alpha = a1;
            alpha.square();
            alpha.mul_assign(self);
            let mut a0 = alpha;
            a0.frobenius_map(1);
            a0.mul_assign(&alpha);

            let neg1 = Fq2 {
                c0: NEGATIVE_ONE,
                c1: Fq::zero(),
            };

            if a0 == neg1 {
                None
            } else {
                a1.mul_assign(self);

                if alpha == neg1 {
                    a1.mul_assign(&Fq2 {
                        c0: Fq::zero(),
                        c1: Fq::one(),
                    });
                } else {
                    alpha.add_assign(&Fq2::one());
                    // alpha = alpha^((q - 1) / 2)
                    alpha = alpha.pow([
                        0x9e10460b6c3e7ea3,
                        0xcbc0b548b438e546,
                        0xdc2822db40c0ac2e,
                        0x183227397098d014,
                    ]);
                    a1.mul_assign(&alpha);
                }

                Some(a1)
            }
        }
    }
}

#[test]
fn test_fq2_get_b() {
    use ff::Field;

    let mut a = Fq2::one();
    a.mul_by_nonresidue();
    let mut b = a.inverse().unwrap();
    let c = b;
    b.double();
    b.add_assign(&c);

    print!("B coeff in Fq2 = {}\n", b);
}

#[test]
fn test_fq2_frobc1() {
    use ff::Field;

    let mut a = Fq2::one();
    a.mul_by_nonresidue();

    let res1 = a.pow([
        0x69602eb24829a9c2,
        0xdd2b2385cd7b4384,
        0xe81ac1e7808072c9,
        0x10216f7ba065e00d,
    ]);
    print!("Frob1 = {}\n", res1);
    
    let res2 = a.pow([
        0x691c1d8b62747890,
        0x8cab57b9adf8eb00,
        0x18c55d8979dcee49,
        0x56cd8a31d35b6b98,
        0xb7a4a8c966ece684,
        0xe5592c705cbd1cac,
        0x1dde2529566d9b5e,
        0x030c96e827699534,
            ]);
    print!("Frob2 = {}\n", res2);

    let res3 = a.pow([
        0x3de6332b975d69b2,
        0x587b5b2bd890e101,
        0x16677d4bec77bbeb,
        0x3fdfdba3309dd645,
        0xdfd4137cd943954b,
        0xcfb035047f38c226,
        0x01b5daf7ac73104c,
        0x4cce8699d63e4f06,
        0x40c0b41264a4b9f4,
        0x7806da9ba1f6d7fb,
        0x110a40708107d53a,
        0x00938e25ae57c88f,
        ]);
    print!("Frob3 = {}\n", res3);

    let res4 = a.pow([
        0xabb30419f6bee420,
        0x6ce183e5f2f8d3b9,
        0x9db42a441998ac99,
        0xf74b04aa96e3852f,
        0x64de4542a9807c06,
        0x41f83258fd90abd1,
        0x5ecb5383626aeca3,
        0xb60804ce8f24ca82,
        0xd4b3aadc1344e8bb,
        0x436b70833cb2615b,
        0x1a87eeb627861611,
        0x4e155ea3e5090666,
        0xacfcff9291a10112,
        0x1cba0005b295d5bc,
        0x319c8e7f94b31729,
        0x001be477ceef2455,
        ]);
    print!("Frob4 = {}\n", res4);

    let res5 = a.pow([
        0x7501aa71de0e8ea2,
        0x97516fd7ca83b8fe,
        0x7da14ac0c03d4182,
        0xaf5d35dc7f80498d,
        0xb257f7f84fb899e0,
        0x372cb1bd547dbe69,
        0xb6696efbf52d5146,
        0x03b6707d4a42574c,
        0xeae6c62cf1670269,
        0xfe70626cbbb760e9,
        0xfa9d12d01fb42086,
        0xc85218d5a7af23b7,
        0x0a70a73464ed35fb,
        0x878713d44d9a2aca,
        0xc81d8fc5cdfe15ee,
        0xa3ebe919611e544d,
        0xfe46bd734126775c,
        0x06f8a7579371f67f,
        0xa94a371ceb68884c,
        0x000545c441ba73d6,
        ]);
    print!("Frob5 = {}\n", res5);

    let res6 = a.pow([
        0xfc4dae0d07a152b0,
        0x3f383f79f9859a0a,
        0x261f0da312f72ab2,
        0x9cc6b2e6efb101d8,
        0xf45a236f76e806da,
        0x7158062cd79d6743,
        0x8adabccc870f23db,
        0x24428ff02b7988c1,
        0x8f55fa0a7ecfa21d,
        0xd5574a8dc73fdcc2,
        0xb86f06772524e5ca,
        0xb4b11653b762bd0f,
        0xb84ec7291c154c58,
        0x2a095f1259f99fb5,
        0x6ccb38fbc9f54a74,
        0x3a3f77faca5c2ea0,
        0x21a469bdd36b9656,
        0x0fa2e41314b53258,
        0xf8ca5207cb9f028e,
        0x489fbf415ec8104e,
        0x711aafe44a1ab611,
        0xfb508020969bab31,
        0xb8b71e4e258cf968,
        0x0000ff25aa9c2350,
        ]);
    print!("Frob6 = {}\n", res6);
}

#[test]
fn test_fq2_frobc2() {
    use ff::Field;

    let mut a = Fq2::one();
    a.mul_by_nonresidue();

    let res1 = a.pow([
        0xd2c05d6490535384,
        0xba56470b9af68708,
        0xd03583cf0100e593,
        0x2042def740cbc01b,
    ]);
    print!("Frob1 = {}\n", res1);
    
    let res2 = a.pow([
        0xd2383b16c4e8f120,
        0x1956af735bf1d600,
        0x318abb12f3b9dc93,
        0xad9b1463a6b6d730,
        0x6f495192cdd9cd08,
        0xcab258e0b97a3959,
        0x3bbc4a52acdb36bd,
        0x06192dd04ed32a68,
            ]);
    print!("Frob2 = {}\n", res2);

    let res3 = a.pow([
        0x7bcc66572ebad364,
        0xb0f6b657b121c202,
        0x2ccefa97d8ef77d6,
        0x7fbfb746613bac8a,
        0xbfa826f9b2872a96,
        0x9f606a08fe71844d,
        0x036bb5ef58e62099,
        0x999d0d33ac7c9e0c,
        0x81816824c94973e8,
        0xf00db53743edaff6,
        0x221480e1020faa74,
        0x01271c4b5caf911e,
        ]);
    print!("Frob3 = {}\n", res3);

    let res4 = a.pow([
        0x57660833ed7dc840,
        0xd9c307cbe5f1a773,
        0x3b68548833315932,
        0xee9609552dc70a5f,
        0xc9bc8a855300f80d,
        0x83f064b1fb2157a2,
        0xbd96a706c4d5d946,
        0x6c10099d1e499504,
        0xa96755b82689d177,
        0x86d6e1067964c2b7,
        0x350fdd6c4f0c2c22,
        0x9c2abd47ca120ccc,
        0x59f9ff2523420224,
        0x3974000b652bab79,
        0x63391cff29662e52,
        0x0037c8ef9dde48aa,
        ]);
    print!("Frob4 = {}\n", res4);

    let res5 = a.pow([
        0xea0354e3bc1d1d44,
        0x2ea2dfaf950771fc,
        0xfb429581807a8305,
        0x5eba6bb8ff00931a,
        0x64afeff09f7133c1,
        0x6e59637aa8fb7cd3,
        0x6cd2ddf7ea5aa28c,
        0x076ce0fa9484ae99,
        0xd5cd8c59e2ce04d2,
        0xfce0c4d9776ec1d3,
        0xf53a25a03f68410d,
        0x90a431ab4f5e476f,
        0x14e14e68c9da6bf7,
        0x0f0e27a89b345594,
        0x903b1f8b9bfc2bdd,
        0x47d7d232c23ca89b,
        0xfc8d7ae6824ceeb9,
        0x0df14eaf26e3ecff,
        0x52946e39d6d11098,
        0x000a8b888374e7ad,
        ]);
    print!("Frob5 = {}\n", res5);

    let res6 = a.pow([
        0xf89b5c1a0f42a560,
        0x7e707ef3f30b3415,
        0x4c3e1b4625ee5564,
        0x398d65cddf6203b0,
        0xe8b446deedd00db5,
        0xe2b00c59af3ace87,
        0x15b579990e1e47b6,
        0x48851fe056f31183,
        0x1eabf414fd9f443a,
        0xaaae951b8e7fb985,
        0x70de0cee4a49cb95,
        0x69622ca76ec57a1f,
        0x709d8e52382a98b1,
        0x5412be24b3f33f6b,
        0xd99671f793ea94e8,
        0x747eeff594b85d40,
        0x4348d37ba6d72cac,
        0x1f45c826296a64b0,
        0xf194a40f973e051c,
        0x913f7e82bd90209d,
        0xe2355fc894356c22,
        0xf6a100412d375662,
        0x716e3c9c4b19f2d1,
        0x0001fe4b553846a1,
        ]);
    print!("Frob6 = {}\n", res6);
}

#[test]
fn test_fq2_frob12() {
    use ff::Field;

    let mut a = Fq2::one();
    a.mul_by_nonresidue();

    let res1 = a.pow([
        0x34b017592414d4e1,
        0xee9591c2e6bda1c2,
        0xf40d60f3c0403964,
        0x0810b7bdd032f006,
    ]);
    print!("Frob1 = {}\n", res1);
    
    let res2 = a.pow([
        0x348e0ec5b13a3c48,
        0xc655abdcd6fc7580,
        0x0c62aec4bcee7724,
        0x2b66c518e9adb5cc,
        0x5bd25464b3767342,
        0x72ac96382e5e8e56,
        0x0eef1294ab36cdaf,
        0x01864b7413b4ca9a,
            ]);
    print!("Frob2 = {}\n", res2);

    let res3 = a.pow([
        0x9ef31995cbaeb4d9,
        0xac3dad95ec487080,
        0x8b33bea5f63bddf5,
        0x9fefedd1984eeb22,
        0x6fea09be6ca1caa5,
        0x67d81a823f9c6113,
        0x00daed7bd6398826,
        0x2667434ceb1f2783,
        0xa0605a0932525cfa,
        0x3c036d4dd0fb6bfd,
        0x888520384083ea9d,
        0x0049c712d72be447,
        ]);
    print!("Frob3 = {}\n", res3);

    let res4 = a.pow([
        0xd5d9820cfb5f7210,
        0xb670c1f2f97c69dc,
        0xceda15220ccc564c,
        0x7ba582554b71c297,
        0xb26f22a154c03e03,
        0xa0fc192c7ec855e8,
        0x2f65a9c1b1357651,
        0xdb04026747926541,
        0xea59d56e09a2745d,
        0xa1b5b8419e5930ad,
        0x0d43f75b13c30b08,
        0x270aaf51f2848333,
        0x567e7fc948d08089,
        0x8e5d0002d94aeade,
        0x98ce473fca598b94,
        0x000df23be777922a,
        ]);
    print!("Frob4 = {}\n", res4);

    let res5 = a.pow([
        0x3a80d538ef074751,
        0x4ba8b7ebe541dc7f,
        0xbed0a560601ea0c1,
        0x57ae9aee3fc024c6,
        0xd92bfbfc27dc4cf0,
        0x1b9658deaa3edf34,
        0x5b34b77dfa96a8a3,
        0x81db383ea5212ba6,
        0xf573631678b38134,
        0x7f3831365ddbb074,
        0xfd4e89680fda1043,
        0xe4290c6ad3d791db,
        0x0538539a32769afd,
        0x43c389ea26cd1565,
        0xe40ec7e2e6ff0af7,
        0x51f5f48cb08f2a26,
        0xff235eb9a0933bae,
        0x037c53abc9b8fb3f,
        0x54a51b8e75b44426,
        0x0002a2e220dd39eb,
        ]);
    print!("Frob5 = {}\n", res5);

    let res6 = a.pow([
        0x7e26d70683d0a958,
        0x1f9c1fbcfcc2cd05,
        0x130f86d1897b9559,
        0x4e63597377d880ec,
        0xfa2d11b7bb74036d,
        0xb8ac03166bceb3a1,
        0xc56d5e66438791ed,
        0x922147f815bcc460,
        0x47aafd053f67d10e,
        0x6aaba546e39fee61,
        0xdc37833b929272e5,
        0x5a588b29dbb15e87,
        0xdc2763948e0aa62c,
        0x1504af892cfccfda,
        0x36659c7de4faa53a,
        0x1d1fbbfd652e1750,
        0x10d234dee9b5cb2b,
        0x07d172098a5a992c,
        0x7c652903e5cf8147,
        0xa44fdfa0af640827,
        0xb88d57f2250d5b08,
        0x7da840104b4dd598,
        0x5c5b8f2712c67cb4,
        0x00007f92d54e11a8,
        ]);
    print!("Frob6 = {}\n", res6);

    let res7 = a.pow([
        0x75e1bff130efc449,
        0xfb4f505fb284ee15,
        0x57b30efd96c492f5,
        0xfdcb862e4e948b59,
        0x3def467dae8887e2,
        0xa47e3f76b755ca8c,
        0xa63f6ea3debc563a,
        0x115d1111a4fc4be2,
        0xc3b2ece674d74549,
        0xb2099a8141cb8830,
        0x120dc0b8ac63867a,
        0xae0245267985fe96,
        0xed38ce9a40128f1d,
        0xeba67d5d8ffa4939,
        0xbff55f706b0de0f5,
        0xb4f3f86e6f982aed,
        0x062675f8a89bd61d,
        0xa1098fb006a9726c,
        0xe974fc0c7b0e5d9c,
        0x0f10af0bdc56fe9e,
        0x628ca855d5d4ac87,
        0x7bd59e7101d9d82d,
        0xed98625bf5dc71aa,
        0x7ab9b78fdc8558f4,
        0x489b4c8564d6f8d2,
        0xd055177a2fbfcd94,
        0x059f68dd1e0cb392,
        0x0000181d8471f268,
        ]);
    print!("Frob7 = {}\n", res7);

    let res8 = a.pow([
        0x742e7ca156ec6a20,
        0x3fee59e5c3e8de2e,
        0xfdef69cd295152ef,
        0xe4ad8aece2ce3640,
        0x05308778897ea5eb,
        0x0a4fc046ae1c2e50,
        0xb16faf17473bba4a,
        0xd106751c900aadfa,
        0x115301a6c43ba345,
        0x19f012d49d8a716c,
        0x6b9d91b1c2a56cc5,
        0xb77230690204b675,
        0xf6d68e7229980805,
        0xf4263d3b11784a87,
        0x24bb64e5adeaa33d,
        0x684c4ff325fa1c4d,
        0x79a8c6430472e684,
        0x823af8186da5609c,
        0x2087966741a30941,
        0x1876205eaf407912,
        0xa614d3f14990435e,
        0xd405328435bcc8df,
        0x5afac38bad541421,
        0x0706fb9d17dec3d8,
        0xecc747832c3f5f69,
        0xe231b0ffd6651ed5,
        0x45fa8e7ff2a80f15,
        0xdce48166a2ee0170,
        0x305fc72544895a12,
        0x516ac4b20d800019,
        0x826e9ab28689a4d3,
        0x0000048efbc0eaac,
        ]);
    print!("Frob8 = {}\n", res8);

    let res9 = a.pow(&[
        0x64984dce4c07e3c1,
        0x2e2096f441339496,
        0xd50c9bd49d279670,
        0xd52ead3ce3a93422,
        0x426dad5fc6a6779a,
        0x3f9dd6b6f19bc638,
        0x6be503d3981b0db5,
        0x0b222e7512412d2c,
        0x484bd275e77ff0bf,
        0xb357542fb851205b,
        0xd8c995246bf492ff,
        0xc6b92fc3bf2887bc,
        0xcd27cfd0d4499277,
        0x967aa0012f40dcf9,
        0x312baab0f5bc64e3,
        0xe465b3c98a822e05,
        0x3133d12c8828f7b8,
        0x357a20a6a8a244ca,
        0xd40b61719905e5b9,
        0xcc4f1d5e2aed7a75,
        0x7895032e16409563,
        0x536db2a17eb54630,
        0xdd66ae0d2d5ac57e,
        0xe150b5a7f229f541,
        0xd882dbabee789616,
        0x1f380eb8775416ca,
        0x73eca6c1c0abcd02,
        0x8bd4f78c2fe1861e,
        0xc53f421003b18ea2,
        0xcae3f7b5d0591ecb,
        0xbebe6ab21737113e,
        0x838f0df2a5f7f26d,
        0xbc2aa2593b06d88f,
        0x0cb02b95a74a8a0a,
        0x74bd9a7b50725838,
        0x000000dc98741fbf,
        ][..]);
    print!("Frob9 = {}\n", res9);

    let res10 = a.pow(&[
        0xed4127472fd6bc68,
        0x72748872e11c4b47,
        0x9c84e64776edc3f8,
        0x008119b96d78b386,
        0xfb0fbff1c5556968,
        0x5009c51f998020be,
        0xd6e688613527a368,
        0xbe4f27942823152c,
        0xd0f09d15c45fe09e,
        0x7eb531158d2bbea5,
        0x51bbe8e71be2cfd1,
        0xbab37561b8c0c7c4,
        0xd9173b5ab551b267,
        0x05fafd9be4c78781,
        0x61883bc8a78540ee,
        0x7fe7aee3dcb694fb,
        0x0e4e85b12b4ac8a8,
        0x9a0aa13a9ab47a86,
        0xd5a3bd591ae12d4b,
        0x5865cbfabbe53b4d,
        0xf98188a9b0cd490f,
        0x3985ef4af715da43,
        0x573661cd006ced38,
        0x95853a6aaa77d5c1,
        0x165d538f0628b55e,
        0x583e75f890f32cac,
        0x5becf43a08a490b9,
        0x63ed4071c1a8087a,
        0x151d41c7701faa25,
        0x1c661c8e4900b051,
        0x581aa0f552590875,
        0x31bf39ff43375aca,
        0xe27c0f3d11310329,
        0x04071459ef3a42c2,
        0x59a2b029be2d6a1f,
        0x30ef71f271cdbf61,
        0xf3774b177f326e78,
        0x976d79b23e8501c5,
        0x9ed0e138633123c9,
        0x00000029b304ecc1,
        ][..]);
    print!("Frob10 = {}\n", res10);

    let res11 = a.pow(&[
        0xf4e8c249a335ddb9,
        0x965085c9440aef70,
        0xc16d84a741174aef,
        0xbe1a366b81fe0680,
        0x1c65508409269d2f,
        0x185861e9cd07fb21,
        0x26b682d951220b7a,
        0x09f189f5a7b75876,
        0x0f7133ab3ecff7f0,
        0xbf7d1ada5df0b2fd,
        0x4b0df5207414a4b6,
        0xbf6a6941b58966d3,
        0x6a15cc7b6bb0483a,
        0xc338843b8a236597,
        0xc8d724986bc0856f,
        0x1dcb8b084e928e52,
        0x3645ba97c4af9161,
        0x7d257d1abed180d3,
        0x0a66e85068416bdb,
        0x8b745a2aeb2bd27e,
        0xe34f87ec4949ec06,
        0x6ba47fa06f902fd6,
        0x225cd33864121ed2,
        0xea5d91e41a3b068b,
        0x35d2fbc8b7a05f5c,
        0xe5b1e22f3dcbc837,
        0xa9f7bdbee44d8301,
        0xbb7a57512450e143,
        0x2e2ca4188fd4eb5b,
        0x9d512b5d1e158636,
        0xdd18753b03f38ee8,
        0xbbe44db3214b380e,
        0x4534f7b060cca3d2,
        0xcbb0309736f9df06,
        0xfcb01aba828f0678,
        0xe2e4d5dac5cc7917,
        0x6631e85c4224e136,
        0xb6c334bbd109d480,
        0x2608e9c50edc2cdf,
        0x959dba8288258d16,
        0x00d895fc73e207c8,
        0x6b5ce08dc4a7bf13,
        0xb02a4f252d6a301f,
        0x00000007e1e7a192,
        ][..]);
    print!("Frob11 = {}\n", res11);
}

#[test]
fn test_calculate_frob_1() {
    let mut a = Fq2::one();
    a.mul_by_nonresidue();

    // Fq2(u + 9)**(((q^1) - 1) / 3)
    
    print!("(i + 9) = {}\n", a);
}

#[test]
fn test_fq2_ordering() {
    let mut a = Fq2 {
        c0: Fq::zero(),
        c1: Fq::zero(),
    };

    let mut b = a.clone();

    assert!(a.cmp(&b) == Ordering::Equal);
    b.c0.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Less);
    a.c0.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Equal);
    b.c1.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Less);
    a.c0.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Less);
    a.c1.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Greater);
    b.c0.add_assign(&Fq::one());
    assert!(a.cmp(&b) == Ordering::Equal);
}

#[test]
fn test_fq2_basics() {
    assert_eq!(
        Fq2 {
            c0: Fq::zero(),
            c1: Fq::zero(),
        },
        Fq2::zero()
    );
    assert_eq!(
        Fq2 {
            c0: Fq::one(),
            c1: Fq::zero(),
        },
        Fq2::one()
    );
    assert!(Fq2::zero().is_zero());
    assert!(!Fq2::one().is_zero());
    assert!(
        !Fq2 {
            c0: Fq::zero(),
            c1: Fq::one(),
        }.is_zero()
    );
}

#[test]
fn test_fq2_squaring() {
    use super::fq::FqRepr;
    use ff::PrimeField;

    let mut a = Fq2 {
        c0: Fq::one(),
        c1: Fq::one(),
    }; // u + 1
    a.square();
    assert_eq!(
        a,
        Fq2 {
            c0: Fq::zero(),
            c1: Fq::from_repr(FqRepr::from(2)).unwrap(),
        }
    ); // 2u

    let mut a = Fq2 {
        c0: Fq::zero(),
        c1: Fq::one(),
    }; // u
    a.square();
    assert_eq!(a, {
        let mut neg1 = Fq::one();
        neg1.negate();
        Fq2 {
            c0: neg1,
            c1: Fq::zero(),
        }
    }); // -1

}

#[test]
fn test_fq2_legendre() {
    use ff::LegendreSymbol::*;

    assert_eq!(Zero, Fq2::zero().legendre());
    // i^2 = -1
    let mut m1 = Fq2::one();
    m1.negate();
    assert_eq!(QuadraticResidue, m1.legendre());
    m1.mul_by_nonresidue();
    assert_eq!(QuadraticNonResidue, m1.legendre());
}

#[cfg(test)]
use rand::{SeedableRng, XorShiftRng};

#[test]
fn test_fq2_mul_nonresidue() {
    let mut rng = XorShiftRng::from_seed([0x5dbe6259, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let mut nine = Fq::one();
    nine.double();
    nine.double();
    nine.double();
    nine.add_assign(&Fq::one());
    let nqr = Fq2 {
        c0: nine,
        c1: Fq::one(),
    };

    for _ in 0..1000 {
        let mut a = Fq2::rand(&mut rng);
        let mut b = a;
        a.mul_by_nonresidue();
        b.mul_assign(&nqr);

        assert_eq!(a, b);
    }
}

#[test]
fn fq2_field_tests() {
    use ff::PrimeField;

    ::tests::field::random_field_tests::<Fq2>();
    ::tests::field::random_sqrt_tests::<Fq2>();
    ::tests::field::random_frobenius_tests::<Fq2, _>(super::fq::Fq::char(), 13);
}
