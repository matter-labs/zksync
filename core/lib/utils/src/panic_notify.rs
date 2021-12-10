// Built-in deps
// External uses
use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use tokio::task::JoinHandle;
// Local uses

/// If its placed inside thread::spawn closure it will notify channel when this thread panics.
pub struct ThreadPanicNotify(pub mpsc::Sender<bool>);

impl Drop for ThreadPanicNotify {
    fn drop(&mut self) {
        if std::thread::panicking() {
            block_on(self.0.send(true)).unwrap();
        }
    }
}

pub fn spawn_panic_handler() -> (JoinHandle<()>, mpsc::Sender<bool>) {
    let (panic_sender, mut panic_receiver) = mpsc::channel(1);

    let handler = tokio::spawn(async move {
        panic_receiver.next().await.unwrap();
    });
    (handler, panic_sender)
}
