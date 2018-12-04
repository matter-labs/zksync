extern crate time;

use std::io::{Write};
use std::sync::{
    mpsc::{channel, Sender, Receiver}, 
    Arc, 
    atomic::{AtomicUsize, Ordering}
};
use self::time::precise_time_ns;
use std::time::Duration;

static UPDATE_INTERVAL: u64 = 1000_000 * 1000; // ms

pub struct MultiBar {
    n_workers:  u64,

    total:      u64,
    cur:        u64,

    prev:       u64,
    prev_time:  u64,

    total_elapsed: u64,

    step:       Arc<AtomicUsize>,
    tx:         Sender<u64>,
    rx:         Receiver<u64>,
}

pub struct ProgressBar {
    //chunk:  u64,
    acc:    u64,
    step:   Arc<AtomicUsize>,
    tx:     Option<Sender<u64>>,
}

/// Simple efficient thread-safe progress indicator
/// It follows the interface of [https://github.com/a8m/pb](https://github.com/a8m/pb)
impl MultiBar {

    /// Create a new MultiBar for stdout
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self{
            n_workers:  0,
            total:      0,
            cur:        0,
            prev:       0,
            prev_time:  precise_time_ns(),
            total_elapsed: 0,
            step:       Arc::new(AtomicUsize::new(1)),
            tx, 
            rx,
        }
    }

    // Create a ProgressBar for a process of `total` steps
    pub fn create_bar(&mut self, chunk: u64) -> ProgressBar {
        self.n_workers += 1;
        self.total += chunk;
        //println!("step 0 of {}", chunk);
        ProgressBar{
            //chunk,
            acc:    0,
            tx:     Some(Sender::clone(&self.tx)),
            step:   Arc::clone(&self.step),
        }
    }

    /// Start listening for updates from ProgressBars in different threads
    pub fn listen(&mut self) {
        //println!("");
        for d in &self.rx {
            if d == 0 {
                self.n_workers -= 1;
            }
            if self.n_workers == 0 { 
                break; 
            }

            self.cur += d;
            let processed = self.cur - self.prev;
            if processed > self.step.load(Ordering::Acquire) as u64 * self.n_workers {
                let now = time::precise_time_ns();
                let elapsed = now - self.prev_time;

                if elapsed > UPDATE_INTERVAL {
                    self.prev = self.cur;
                    self.prev_time = now;
                    self.total_elapsed += elapsed;

                    print!("\rprocessed {:2}%: {} of {}.", self.cur * 100 / self.total, self.cur, self.total);

                    let r = Duration::from_nanos((self.total - self.cur) * self.total_elapsed / self.cur).as_secs();
                    print!(" Remaining estimated: {} h {} min {} s", r / 3600, r % 3600 / 60, r % 60);

                    let new_step = (self.cur * UPDATE_INTERVAL / self.total_elapsed) / self.n_workers;
                    self.step.store(new_step as usize, Ordering::Release);
                    
                    std::io::stdout().flush().unwrap();
                }
            }

        }
        println!("\rdone                                                                   ");
    }
}

impl ProgressBar {

    /// Increment progress by `d` steps
    pub fn add(&mut self, d: u64) {
        self.acc += d;
        if self.acc > (self.step.load(Ordering::Relaxed) as u64) {
            if let Some(tx) = &self.tx { 
                tx.send(self.acc).unwrap(); 
            }
            self.acc = 0;
        }
    }

    /// Finish the process
    pub fn finish(&mut self) {
        let tx = self.tx.take().unwrap();
        tx.send(0).unwrap();
        drop(tx);
    }
}

#[test]
fn test_progress_display() {

    let mut mb = MultiBar::new();

    for _j in 1..=0 { 
        let mut pb = mb.create_bar(3600000); 
        std::thread::spawn(move || {
            for _i in 0..3600000 {
                std::thread::sleep(Duration::from_millis(1));
                pb.add(1);
            }
            pb.finish();
        });
    };
    //mb.listen();
}
