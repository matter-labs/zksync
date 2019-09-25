#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;

pub mod api_server;
pub mod committer;
pub mod eth_sender;
pub mod eth_watch;
pub mod state_keeper;

/// If its placed inside thread::spawn closure it will notify channel when this thread panics.
pub struct ThreadPanicNotify(pub std::sync::mpsc::Sender<bool>);

impl Drop for ThreadPanicNotify {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.send(true).unwrap();
        }
    }
}
