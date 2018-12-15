use ff::{Field, PrimeField, PrimeFieldDecodingError, PrimeFieldRepr};

#[derive(PrimeField, Serialize, Deserialize)]
#[PrimeFieldModulus = "21888242871839275222246405745257275088548364400416034343698204186575808495617"]
#[PrimeFieldGenerator = "7"]
pub struct Fr(FrRepr);

// extern crate hex;
// #[test]
// fn test_fr_serialize() {
//     let value = &Fr::one();
//     let mut buf: Vec<u8> = vec![];
//     value.into_repr().write_be(&mut buf).unwrap();
//     println!("{}", hex::encode(&buf));
// }
// #[test]
// fn test_fr_deserialize() {

//     let s: &str = "00000000000000000000000000000000000000000000000000000000000000a7";
//     let buf = hex::decode(&s).unwrap();
    
//     let mut repr = <Fr as PrimeField>::Repr::default();
//     repr.read_be(&buf[..]).unwrap();
//     println!("{:?}", repr);

//     let fr = Fr::from_repr(repr).unwrap();
//     println!("{:?}", fr.into_repr());
// }

#[test]
fn test_roots_of_unity() {
    assert_eq!(Fr::S, 28);
}