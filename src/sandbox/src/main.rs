#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate bigdecimal;
extern crate ff;

use bigdecimal::BigDecimal;
use std::str::FromStr;

use ff::Field;

extern crate models;
extern crate plasma;

use models::{Operation, Action};
use plasma::models::{Block, BlockData, Fr, TransferTx, TxSignature};

#[derive(Clone, Serialize, Deserialize)]
struct Test {
    a: u128,
    b: BigDecimal,
}

#[derive(Serialize, Deserialize)]
pub struct TT {
    pub from:               u32,
    pub to:                 u32,
    // pub amount:             BigDecimal,
    // pub fee:                BigDecimal,
    pub nonce:              u32,
    pub good_until_block:   u32,
    // pub signature:          TxSignature,
}

#[derive(Serialize, Deserialize)]
pub struct LT {
    pub l: Vec<TransferTx>
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BlockData2 {
    Transfer{
        //#[serde(skip)]
        transactions:   Vec<TT>,
        total_fees:     String,
    },
    Deposit{
        //#[serde(skip)]
        //transactions: Vec<DepositTx>, 
        batch_number: u32,
    },
    Exit{
        //#[serde(skip)]
        //transactions: Vec<ExitTx>, 
        batch_number: u32,
    }
}

fn main() {


    // let tx0 = TT{
    //     from:               1,
    //     to:                 2,
    //     amount:             BigDecimal::from(0),
    //     fee:                BigDecimal::from(0),
    //     nonce:              3,
    //     good_until_block:   4,
    //     signature:          TxSignature{
    //         r_x:    Fr::zero(),
    //         r_y:    Fr::zero(),
    //         s:    Fr::zero(),
    //     },
    //     //cached_pub_key: None
    // };

    let s = r#"{"l": [{"from":1,"to":2,"amount":"0","fee":"0","nonce":3,"good_until_block":4,"signature":{"r_x":"0x0000000000000000000000000000000000000000000000000000000000000000","r_y":"0x0000000000000000000000000000000000000000000000000000000000000000","s":"0x0000000000000000000000000000000000000000000000000000000000000000"}}]}"#;
    let op: LT = serde_json::from_str(&s).expect("00");

    println!("done");
    //return;



    let op0 = Operation{
        action: Action::Commit, 
        block:  Block{
            block_number:   0,
            new_root_hash:  Fr::zero(),
            block_data: BlockData::Transfer{
                total_fees:     BigDecimal::from(0),
                transactions:   vec![
                    TransferTx{
                        from:               1,
                        to:                 2,
                        amount:             BigDecimal::from(0),
                        fee:                BigDecimal::from(0),
                        nonce:              3,
                        good_until_block:   4,
                        signature:          TxSignature{
                            r_x:    Fr::zero(),
                            r_y:    Fr::zero(),
                            s:    Fr::zero(),
                        },
                        cached_pub_key: None
                    }
                ],
            }
        }, 
        accounts_updated: None, 
        tx_meta: None
    };

    let s = serde_json::to_string(&op0).expect("to_string");
    println!("op = {}", s);

    let s = r#"{"action":{"type":"Commit"},"block":{"block_number":0,"new_root_hash":"0x0000000000000000000000000000000000000000000000000000000000000000","block_data":
        {"type":"Transfer","transactions":[
            {"from":1,"to":2,"amount":"0","fee":"0","nonce":3,"good_until_block":4,"signature":{"r_x":"0x0000000000000000000000000000000000000000000000000000000000000000","r_y":"0x0000000000000000000000000000000000000000000000000000000000000000","s":"0x0000000000000000000000000000000000000000000000000000000000000000"}}
            ],"total_fees":"0"}
        },"accounts_updated":null}"#;

    let s2 = r#"{"type":"Transfer","transactions":[
        {"from":1,"to":2,"amount":"0","fee":"0","nonce":3,"good_until_block":4,"signature":{"r_x":"0x0000000000000000000000000000000000000000000000000000000000000000","r_y":"0x0000000000000000000000000000000000000000000000000000000000000000","s":"0x0000000000000000000000000000000000000000000000000000000000000000"}}
        ],"total_fees":"0"}"#;

    let op: BlockData2 = serde_json::from_str(&s2).expect("BlockData");
//    return;

    let s1 = r#"{
       "accounts_updated":null,
       "action":{"type":"Commit"},
       "block":{
          "block_data":{
             "total_fees":"0",


             
             "transactions":[                
                {
                   "amount":"100",
                   "fee":"0",
                   "from":5,
                   "good_until_block":50000,
                   "nonce":1,
                   "signature":{
                      "r_x":"0x09e63b611213ce2da215e7a003ef5bfb0da29d2ee3022f4eaa450b664143d7d0",
                      "r_y":"0x1f60b3521f326df2d2009d37d1a0eefaa80fd816e3bc61d0e660f75e74e4ebac",
                      "s":"0x05ec22e005f0c3fe95ae2f2034983d0600efc44f99f6fb042af03d52d3a9ab85"
                   },
                   "to":4
                }
             ],
             "type":"Transfer"
          },
          "block_number":8,
          "new_root_hash":"0x2bcba4542210660958161160d3f340db90fa03c528c347cdb3a16a39f62836b8"
       }
    }"#;

    // let s2 = r#"{
    //       "type":"Commit"
    //    }"#;
    
    // let action: Action = serde_json::from_str(&s2).expect("Action");

    let op: Operation= serde_json::from_str(&s).expect("Operation0");
    let op: Operation = serde_json::from_str(&s1).expect("Operation");

    let t0 = Test{a: 3, b: BigDecimal::from_str("7").expect("1")};
    let s = serde_json::to_string(&t0).expect("to_string");
    println!("s = {}", s);

    // let v = serde_json::to_value(t0).expect("to_value");
    let s = r#"{"a": 1, "b": "2"}"#; //v.to_string();
    let t: Test = serde_json::from_str(&s).expect("3");

    println!("test");
}