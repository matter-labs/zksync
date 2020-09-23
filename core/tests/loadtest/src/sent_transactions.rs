use models::node::tx::TxHash;

#[derive(Debug)]
pub struct SentTransactions {
    pub op_serial_ids: Vec<u64>,
    pub tx_hashes: Vec<TxHash>,
}

impl SentTransactions {
    pub fn merge(&mut self, other: SentTransactions) {
        self.op_serial_ids.extend(other.op_serial_ids.into_iter());
        self.tx_hashes.extend(other.tx_hashes.into_iter());
    }

    pub fn new() -> SentTransactions {
        SentTransactions {
            op_serial_ids: Vec::new(),
            tx_hashes: Vec::new(),
        }
    }

    pub fn add_op_id(&mut self, v: u64) {
        self.op_serial_ids.push(v);
    }

    pub fn add_tx_hash(&mut self, v: TxHash) {
        self.tx_hashes.push(v);
    }

    pub fn len(&self) -> usize {
        self.op_serial_ids.len() + self.tx_hashes.len()
    }
}
