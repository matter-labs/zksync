use crate::eip712_signature::{EncodedStructureMember, StructMember};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use zksync_basic_types::H256;

/// Interface that collects members of the structure into the structure of the EIP-712 standard.
pub trait StructBuilder {
    fn new() -> Self;

    fn add_member<MEMBER: crate::eip712_signature::StructMember>(
        &mut self,
        name: &str,
        member: &MEMBER,
    );
}

/// Builder for collecting information about types of nested structures.
pub(crate) struct TypeBuilder {
    members: Vec<EncodedStructureMember>,
}

impl TypeBuilder {
    pub fn get_inner_members(&self) -> Vec<EncodedStructureMember> {
        self.members.clone()
    }
}

impl StructBuilder for TypeBuilder {
    fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    fn add_member<MEMBER: StructMember>(&mut self, name: &str, member: &MEMBER) {
        self.members
            .push(EncodedStructureMember::encode(name, member));
    }
}

struct OuterTypeBuilder {
    inner_members_queue: VecDeque<EncodedStructureMember>,
}

impl OuterTypeBuilder {
    fn new() -> Self {
        Self {
            inner_members_queue: VecDeque::new(),
        }
    }

    fn add_member(&mut self, encoded_member: EncodedStructureMember) {
        // If the type is not used by the structure, then it is possible not
        // to process it as it is not included in the list of types of nested structures.
        if encoded_member.is_reference_type {
            self.inner_members_queue.push_back(encoded_member);
        }
    }

    fn build(mut self) -> BTreeMap<String, EncodedStructureMember> {
        // All nested structures must be added to the encoded type alphabetically,
        // so we will support a red-black tree with a key by the name of the structure type.
        let mut result = BTreeMap::new();

        while let Some(front_element) = self.inner_members_queue.pop_front() {
            if result.get(&front_element.member_type).is_some() {
                continue;
            }

            result.insert(front_element.member_type.clone(), front_element.clone());
            for inner_member in front_element.inner_members {
                if inner_member.is_reference_type && result.get(&inner_member.member_type).is_none()
                {
                    self.inner_members_queue.push_back(inner_member);
                }
            }
        }
        result
    }
}

// Builder that encodes type information and structure data for for hashing the structure according to the EIP-712 standard.
pub(crate) struct EncodeBuilder {
    members: Vec<(EncodedStructureMember, H256)>,
}

impl EncodeBuilder {
    /// Returns the concatenation of the encoded member values in the order that they appear in the type.
    pub fn encode_data(&self) -> Vec<H256> {
        // encodeData(s : 𝕊) = enc(value₁) ‖ enc(value₂) ‖ … ‖ enc(valueₙ).
        self.members.iter().map(|(_, data)| *data).collect()
    }

    /// Return the encoded structure type as `name ‖ "(" ‖ member₁ ‖ "," ‖ member₂ ‖ "," ‖ … ‖ memberₙ ")"`.
    ///
    /// If the struct type references other struct types (and these in turn reference even more struct types),
    /// then the set of referenced struct types is collected, sorted by name and appended to the encoding.
    pub fn encode_type(&self, type_name: &str) -> String {
        let mut result = String::new();

        let mut outer_members_builder = OuterTypeBuilder::new();
        for (member, _) in self.members.iter() {
            outer_members_builder.add_member(member.clone());
        }
        let outer_members = outer_members_builder.build();

        // Collecting all members of the structure as a coded structure.
        let inner_member = {
            let member_type = type_name.to_string();
            let inner_members = self
                .members
                .iter()
                .cloned()
                .map(|(encoded_struct, _)| encoded_struct)
                .collect::<Vec<_>>();

            EncodedStructureMember {
                member_type,
                name: String::default(),
                is_reference_type: true,
                inner_members,
            }
        };

        result.push_str(&inner_member.get_encoded_type());
        for (_, outer_member) in outer_members {
            result.push_str(&outer_member.get_encoded_type());
        }

        result
    }

    /// Return the encoded structure type as `{ member_type: [{"name": member_name₁, "type": member_type₁}, ...] }`.
    ///
    /// If the struct type references other struct types (and these in turn reference even more struct types),
    /// then the set of referenced struct types is collected, sorted by name and appended to the encoding.
    pub fn get_json_types(&self, type_name: &str) -> Vec<Value> {
        let mut result = Vec::new();

        let mut outer_members_builder = OuterTypeBuilder::new();
        for (member, _) in self.members.iter() {
            outer_members_builder.add_member(member.clone());
        }
        let outer_members = outer_members_builder.build();

        // Collecting all members of the structure as a coded structure.
        let inner_member = {
            let member_type = type_name.to_string();
            let inner_members = self
                .members
                .iter()
                .cloned()
                .map(|(encoded_struct, _)| encoded_struct)
                .collect::<Vec<_>>();

            EncodedStructureMember {
                member_type,
                name: String::default(),
                is_reference_type: true,
                inner_members,
            }
        };

        result.push(inner_member.get_json_types());
        for (_, outer_member) in outer_members {
            result.push(outer_member.get_json_types());
        }

        result
    }
}

impl StructBuilder for EncodeBuilder {
    fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    fn add_member<MEMBER: StructMember>(&mut self, name: &str, member: &MEMBER) {
        let encoded_data = member.encode_member_data();
        self.members
            .push((EncodedStructureMember::encode(name, member), encoded_data));
    }
}
