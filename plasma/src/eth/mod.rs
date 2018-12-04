use rustc_hex::FromHex;

use web3::futures::{Future, Stream};
use web3::contract::{Contract, Options, CallFuture};
use web3::types::{Address, U256, H256, U128, Bytes};
use web3::transports::{EventLoopHandle, Http};

pub struct Client {
    event_loop: EventLoopHandle,
    web3:       web3::Web3<Http>,
    contract:   Contract<Http>,
    my_account: Address,
}

pub type U32 = u64; // because missing in web3::types; u64 is fine since only used for tokenization

type ABI = (&'static [u8], &'static str);

pub const TEST_PLASMA_ALWAYS_VERIFY: ABI = (
    include_bytes!("../../contracts/bin/contracts_Plasma_sol_PlasmaTest.abi"),
    include_str!("../../contracts/bin/contracts_Plasma_sol_PlasmaTest.bin"),
);

pub const PROD_PLASMA: ABI = (
    include_bytes!("../../contracts/bin/contracts_Plasma_sol_Plasma.abi"),
    include_str!("../../contracts/bin/contracts_Plasma_sol_Plasma.bin"),
);

// all methods are blocking and panic on error for now
impl Client {

    pub fn new(contract_abi: ABI) -> Self {
        // TODO: check env vars to decide local/testnet/live
        Self::new_local(contract_abi)
    }

    fn new_local(contract_abi: ABI) -> Self {

        let (event_loop, transport) = Http::new("http://localhost:8545").unwrap();
        let web3 = web3::Web3::new(transport);

        let accounts = web3.eth().accounts().wait().unwrap();
        let my_account = accounts[0];

        // Get the contract bytecode for instance from Solidity compiler
        let bytecode: Vec<u8> = contract_abi.1.from_hex().unwrap();

        // Deploying a contract
        let contract = Contract::deploy(web3.eth(), contract_abi.0)
            .unwrap()
            .confirmations(0)
            .options(Options::with(|opt| {
                opt.gas = Some(7000_000.into())
            }))
            .execute(bytecode, (), my_account,
            )
            .expect("Correct parameters are passed to the constructor.")
            .wait()
            .unwrap();
        
        //println!("contract: {:?}", contract);
 
        Self{event_loop, web3, contract, my_account}
    }

    fn new_testnet() -> Self {
        unimplemented!()

        // TODO: change to infura
        // let (_eloop, transport) = web3::transports::Http::new("http://localhost:8545").unwrap();
        // let web3 = web3::Web3::new(transport);

        // TODO: read contract addr from env var
        // let contract_address = "664d79b5c0C762c83eBd0d1D1D5B048C0b53Ab58".parse().unwrap();
        // let contract = Contract::from_json(
        //     web3.eth(),
        //     contract_address,
        //     include_bytes!("../../contracts/build/bin/contracts_Plasma_sol_Plasma.abi"),
        // ).unwrap();

        // TODO: read account and secret from env var
    }

    /// Returns tx hash
    pub fn commit_block(
        &self, 
        block_num: U32, 
        total_fees: U128, 
        tx_data_packed: Vec<u8>, 
        new_root: H256) -> Result<H256, web3::contract::Error>
    {
        self.contract
            .call("commitBlock", 
                (block_num, total_fees, tx_data_packed, new_root), 
                self.my_account, 
                Options::with(|opt| {
                    opt.gas = Some(3000_000.into())
                }))
            .then(|tx| {
                println!("got tx: {:?}", tx);
                tx
            }).wait()
    }

    /// Returns tx hash
    pub fn verify_block(
        &self, 
        block_num: U32, 
        proof: [U256; 8]) -> Result<H256, web3::contract::Error>
    {
        self.contract
            .call("verifyBlock", 
                (block_num, proof), 
                self.my_account, 
                Options::with(|opt| {
                    opt.gas = Some(3000_000.into())
                }))
            .then(|tx| {
                println!("got tx: {:?}", tx);
                tx
            }).wait()
    }
}

#[test]
fn test_web3() {

    let client = Client::new(TEST_PLASMA_ALWAYS_VERIFY);

    let block_num: u64 = 0;
    let total_fees: U128 = U128::from_dec_str("0").unwrap();
    let tx_data_packed: Vec<u8> = vec![];
    let new_root: H256 = H256::zero();

    let proof: [U256; 8] = [U256::zero(); 8];

    println!("committing block...");
    assert!(client.commit_block(block_num, total_fees, tx_data_packed, new_root).is_ok());
    println!("verifying block...");
    assert!(client.verify_block(block_num, proof).is_ok());
}