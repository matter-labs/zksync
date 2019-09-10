use models::node::tx::TxSignature;

fn main() {
    let inp = r#"{"pub_key":"895fda3accc92f0939afa744f0712c068078c573ecfd3e42f0ffc2ca0840220f","sign":"ac213f1ee98750a529b6ed337ffc55e0845d138d501a631ac47d72962796b428843370ef786b69e6a94de49269597d4ebd6a0c6b0454d626eed54a7bc76ed305"}"#;
    let sign: TxSignature = serde_json::from_str(&inp).expect("json deser error");
    sign.verify_musig_pedersen(&[1, 2, 3])
        .expect("must be correct");
    assert!(sign.verify_musig_sha256(&[1, 2, 3]).is_none());
}
