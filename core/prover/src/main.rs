// Built-in uses
use std::env;
use std::sync::{atomic::AtomicBool, Arc, mpsc};
use std::thread;
// External uses
use signal_hook::iterator::Signals;
// Workspace uses
use prover;

fn main() {
    // TODO: add logging.
    // TODO: add docstrings.
//    let (tx, rx) = mpsc::channel::<()>();
//    // handle terminate signals like on ctrl+c
//    start_handle_terminate_signals_thread(tx);
//    new_prover().run(rx);
}

//fn start_handle_terminate_signals_thread(tx: mpsc::Sender<()>) {
//    let signal = Arc::new(AtomicBool::new(false));
//    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&signal))
//        .expect("Error setting SIGTERM handler");
//    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&signal))
//        .expect("Error setting SIGINT handler");
//    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&signal))
//        .expect("Error setting SIGQUIT handler");
//
//    let signals = Signals::new(&[
//        signal_hook::SIGTERM,
//        signal_hook::SIGINT,
//        signal_hook::SIGQUIT,
//    ])
//    .expect("Signals::new() failed");
//    thread::spawn(move || {
//        for _ in signals.forever() {
//            tx.send(()).unwrap();
//            // TODO: notify server of provers stop.
//        }
//    });
//}
//
//fn new_prover() -> Prover {
//    let worker = env::var("POD_NAME").unwrap_or_else(|_| "default".to_string());
//    let key_dir = std::env::var("KEY_DIR").expect("KEY_DIR not set");
//    Prover::new(worker, key_dir)
//}

//#[macro_use]
//// Built-in uses
//use std::env;
//use std::sync::{atomic::AtomicBool, Arc};
//use std::thread;
//// External uses
//use env_logger;
//use log::info;
//use signal_hook::iterator::Signals;
//use tokio::runtime::current_thread::Runtime;
//use tokio::sync::oneshot;
//// Workspace uses
//use prover::BabyProver;
//use storage::StorageProcessor;
//
//fn main() {
//    env_logger::init();
//
//    // handle ctrl+c
//    let stop_signal = Arc::new(AtomicBool::new(false));
//    signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&stop_signal))
//        .expect("Error setting SIGTERM handler");
//    signal_hook::flag::register(signal_hook::SIGINT, Arc::clone(&stop_signal))
//        .expect("Error setting SIGINT handler");
//    signal_hook::flag::register(signal_hook::SIGQUIT, Arc::clone(&stop_signal))
//        .expect("Error setting SIGQUIT handler");
//
//    let worker = env::var("POD_NAME").unwrap_or_else(|_| "default".to_string());
//    let key_dir = std::env::var("KEY_DIR").expect("KEY_DIR not set");
//    info!("creating prover, worker: {}", worker);
//    let mut prover = BabyProver::create(worker, key_dir).unwrap();
//    let prover_id = prover.prover_id;
//
//    let mut rt = Runtime::new().unwrap();
//    let (shutdown_tx, shutdown_rx) = oneshot::channel();
//    prover.start_timer_interval(&rt.handle());
//
//    // Run tokio timeline in a new thread
//    thread::spawn(move || {
//        prover.run(shutdown_tx, stop_signal);
//    });
//
//    let signals = Signals::new(&[
//        signal_hook::SIGTERM,
//        signal_hook::SIGINT,
//        signal_hook::SIGQUIT,
//    ])
//    .expect("Signals::new() failed");
//    thread::spawn(move || {
//        for _ in signals.forever() {
//            info!(
//                "Termination signal received. Prover will finish the job and shut down gracefully"
//            );
//            let storage =
//                StorageProcessor::establish_connection().expect("db connection failed for prover");
//            storage.record_prover_stop(prover_id).expect("db failed");
//        }
//    });
//
//    rt.block_on(shutdown_rx).unwrap();
//}
