use zksync_types::ZkSyncOp;

fn main() {
    let hex_data = std::env::args()
        .nth(1)
        .expect("cli arg should be hex of pubdata");
    let data = hex::decode(&hex_data).expect("failed to decode hex");

    let mut unparsed_data = data.as_slice();
    while !unparsed_data.is_empty() {
        let op_type = unparsed_data[0];
        let op_data_len = ZkSyncOp::public_data_length(op_type).expect("wrong op type");
        assert!(
            data.len() > op_data_len,
            "not enough bytes in the pubdata for current op"
        );
        let (current_op, unparsed) = unparsed_data.split_at(op_data_len);
        let op = ZkSyncOp::from_public_data(&current_op).expect("failed to parse pubdata");
        println!("{:#?}", op);
        unparsed_data = unparsed;
    }
}
