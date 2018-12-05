extern crate tokio;
extern crate futures;
extern crate plasma;

use futures::future;
use futures::future::lazy;

use plasma::server::account_manager::AccountManager;

fn main() {
    println!("starting the server");
    tokio::run(lazy( || {
        let _acc_man = AccountManager::new();
        future::ok(())
    }));
}