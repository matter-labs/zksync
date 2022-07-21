use parity_crypto::Keccak256;

use zksync_basic_types::{Address, H256, U256};

use crate::eip712_signature::typed_structure::{EncodedStructureMember, StructMember};

impl StructMember for String {
    const MEMBER_TYPE: &'static str = "string";
    const IS_REFERENCE_TYPE: bool = false;

    fn get_inner_members(&self) -> Vec<EncodedStructureMember> {
        Vec::new()
    }

    fn encode_member_data(&self) -> H256 {
        self.keccak256().into()
    }
}

impl StructMember for Address {
    const MEMBER_TYPE: &'static str = "address";
    const IS_REFERENCE_TYPE: bool = false;

    fn get_inner_members(&self) -> Vec<EncodedStructureMember> {
        Vec::new()
    }

    fn encode_member_data(&self) -> H256 {
        H256::from(*self)
    }
}

impl<const N: usize> StructMember for [u8; N] {
    const MEMBER_TYPE: &'static str = "bytes";
    const IS_REFERENCE_TYPE: bool = false;
    const ASSERT: usize = 31 - N;

    fn member_type(&self) -> String {
        format!("{}{}", Self::MEMBER_TYPE, N)
    }

    fn get_inner_members(&self) -> Vec<EncodedStructureMember> {
        Vec::new()
    }

    #[allow(path_statements)]
    fn encode_member_data(&self) -> H256 {
        #[allow(clippy::no_effect)]
        // It's compile time check
        Self::ASSERT;
        let mut bytes = [0; 32];
        bytes[..N].copy_from_slice(self);
        bytes.into()
    }
}

impl StructMember for &[u8] {
    const MEMBER_TYPE: &'static str = "bytes";
    const IS_REFERENCE_TYPE: bool = false;

    fn get_inner_members(&self) -> Vec<EncodedStructureMember> {
        Vec::new()
    }

    fn encode_member_data(&self) -> H256 {
        self.keccak256().into()
    }
}

impl StructMember for U256 {
    const MEMBER_TYPE: &'static str = "uint256";
    const IS_REFERENCE_TYPE: bool = false;

    fn get_inner_members(&self) -> Vec<EncodedStructureMember> {
        Vec::new()
    }

    fn encode_member_data(&self) -> H256 {
        let mut bytes = [0u8; 32];
        self.to_big_endian(&mut bytes);

        bytes.into()
    }
}

impl StructMember for H256 {
    const MEMBER_TYPE: &'static str = "uint256";
    const IS_REFERENCE_TYPE: bool = false;

    fn get_inner_members(&self) -> Vec<EncodedStructureMember> {
        Vec::new()
    }

    fn encode_member_data(&self) -> H256 {
        *self
    }
}

macro_rules! impl_primitive {
    ($T: ident, $name:expr, $bit_size:expr) => {
        impl StructMember for $T {
            const MEMBER_TYPE: &'static str = $name;
            const IS_REFERENCE_TYPE: bool = false;
            fn get_inner_members(&self) -> Vec<EncodedStructureMember> {
                Vec::new()
            }
            fn encode_member_data(&self) -> H256 {
                let mut bytes = [0u8; 32];
                let bytes_value = self.to_be_bytes();
                bytes[32 - $bit_size / 8..].copy_from_slice(&bytes_value);

                bytes.into()
            }
        }
    };
}

impl_primitive!(u8, "uint8", 8);
impl_primitive!(u16, "uint16", 16);
impl_primitive!(u32, "uint32", 32);
impl_primitive!(u64, "uint64", 64);
impl_primitive!(u128, "uint128", 128);
