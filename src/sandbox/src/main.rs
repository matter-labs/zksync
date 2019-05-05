extern crate iron;
extern crate bodyparser;
extern crate persistent;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate tokio;

use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio::timer::Interval;

use std::time::{Duration, Instant};

use persistent::Read;
use iron::typemap::Key;
use iron::status;
use iron::prelude::*;

#[derive(Deserialize, Debug, Clone)]
struct MyStructure {
    a: String,
    b: Option<String>,
}

#[derive(Copy, Clone, Debug)]
pub struct State {
    a: u32
}
impl Key for State { type Value = Self; }


fn log_body(req: &mut Request) -> IronResult<Response> {
    let body = req.get::<bodyparser::Raw>();
    match body {
        Ok(Some(body)) => println!("Read body:\n{}", body),
        Ok(None) => println!("No body"),
        Err(err) => println!("Error: {:?}", err)
    }

    let json_body = req.get::<bodyparser::Json>();
    match json_body {
        Ok(Some(json_body)) => println!("Parsed body:\n{:?}", json_body),
        Ok(None) => println!("No body"),
        Err(err) => println!("Error: {:?}", err)
    }

    let struct_body = req.get::<bodyparser::Struct<MyStructure>>();
    match struct_body {
        Ok(Some(struct_body)) => println!("Parsed body:\n{:?}", struct_body),
        Ok(None) => println!("No body"),
        Err(err) => println!("Error: {:?}", err)
    }

    let arc = req.get::<Read<State>>().unwrap();
    let state = arc.as_ref();
    println!("state = {:?}", state);

    Ok(Response::with(status::Ok))
}

const MAX_BODY_LENGTH: usize = 1024 * 1024 * 10;

// While the example is running, try the following curl commands and see how they are
// logged by the Rust server process:
//
// `curl -i "localhost:3000/" -H "application/json" -d '{"name":"jason","age":"2"}'`
// `curl -i "localhost:3000/" -H "application/json" -d '{"a":"jason","b":"2"}'`
// `curl -i "localhost:3000/" -H "application/json" -d '{"a":"jason"}'`
fn main() {

    let task = Interval::new(Instant::now(), Duration::from_millis(1000))
    //.take(10)
    .for_each(|instant| {
        println!("fire; instant={:?}", instant);
        Ok(())
    })
    .map_err(|e| panic!("interval errored; err={:?}", e));

    // Create the runtime
    let mut rt = Runtime::new().unwrap();
    rt.spawn(task);
    //tokio::run(task);

    let mut chain = Chain::new(log_body);
    chain.link_before(Read::<bodyparser::MaxBodyLength>::one(MAX_BODY_LENGTH));
    chain.link(Read::<State>::both(State{a: 5}));
    Iron::new(chain).http("localhost:3000").unwrap();
}