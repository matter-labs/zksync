#[macro_use]
extern crate serde_derive;
extern crate serde_json;

#[derive(Clone, Serialize, Deserialize)]
struct Test {
    a: u128
}

fn main() {
    //let s = "{a: 1}";
    let t0 = Test{a: 3};
    let v = serde_json::to_value(t0).unwrap();
    let t: Test = serde_json::from_value(v).unwrap();
    println!("test");
}