// Built-in deps.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const DEFAULT_REPORT_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_THRESHOLD: f64 = 5.0f64;

/// Metrics counter is a helper structure which can be used to quickly measure
/// performance of some repetitive action (e.g. TPS in state keeper).
///
/// This structure is expected to be used for a quick-and-dirty real-life benches.
///
/// Reports are output via `log::info` by invoking `output_stats` method. The interval
/// between messages can be configured by a constructor parameters, so the consecutive
/// calls to that method won't spam the stats, but rather will only display message if
/// the required amount of time had passed.
///
/// Example:
///
/// ```rust
/// use zksync_api::utils::metrics_counter::MetricsCounter;
/// use std::time::Duration;
///
/// struct SomeProcessor {
///     pub metrics_counter: MetricsCounter,
/// }
///
/// impl SomeProcessor {
///     pub fn new() -> Self {
///         Self {
///             metrics_counter: MetricsCounter::new(
///                 "SomeProcessor metrics", // Prefix for a message.
///                 0.0f64, // Noise threshold.
///                 Duration::from_millis(100), // Report interval.
///             ),
///         }
///     }
///
///     pub fn do_work(&mut self, elements_to_process: Vec<i32>) {
///         // Do some work...
///         self.metrics_counter.add_samples(elements_to_process.len());
///     }
/// }
///
/// env_logger::init();
/// let mut processor = SomeProcessor::new();
/// for _ in 0..5 {
///     let elements = // ...
///         #  vec![1, 2, 3];
///     processor.do_work(elements);
///     processor.metrics_counter.output_stats();
/// }
/// ```
#[derive(Debug)]
pub struct MetricsCounter {
    message_prefix: String,
    noise_threshold: f64,
    report_interval: Duration,
    num_of_els: usize,
    last_seen_num_of_els: usize,
    time_of_last_check_of_els: Instant,
    stats: StatsCounter,
}

impl Default for MetricsCounter {
    fn default() -> Self {
        Self {
            message_prefix: String::new(),
            noise_threshold: DEFAULT_THRESHOLD,
            report_interval: DEFAULT_REPORT_INTERVAL,
            num_of_els: 0,
            last_seen_num_of_els: 0,
            time_of_last_check_of_els: Instant::now(),
            stats: StatsCounter::new(),
        }
    }
}
impl MetricsCounter {
    /// Creates a new counter.
    ///
    /// Constructor parameters:
    ///
    /// - `message_prefix`: prefix to be output at the beginning of each stats report.
    /// - `noise_threshold`: threshold to cut "garbage" values (e.g. in the
    ///    beginning/end of the measurement).
    /// - `report_interval`: minimum amount of time between printing the stats.
    pub fn new(
        message_prefix: impl ToString,
        noise_threshold: f64,
        report_interval: Duration,
    ) -> Self {
        Self {
            message_prefix: message_prefix.to_string(),
            noise_threshold,
            report_interval,
            ..Default::default()
        }
    }

    /// Adds the provided amount of samples to the counter.
    pub fn add_samples(&mut self, n_els: usize) {
        self.num_of_els += n_els;
    }

    /// Prints stats (current throughput, min / max / avg values).
    /// Does nothing if not enough time has passed since last invocation.
    pub fn output_stats(&mut self) {
        if self.time_of_last_check_of_els.elapsed() > self.report_interval {
            let tps = 1000f64 * (self.num_of_els - self.last_seen_num_of_els) as f64
                / self.time_of_last_check_of_els.elapsed().as_millis() as f64;

            if tps > self.noise_threshold {
                self.stats.add_sample(tps);

                log::info!(
                    "Throughput: {} el/s; min: {}, max: {}, avg: {}",
                    tps,
                    self.stats.min(),
                    self.stats.max(),
                    self.stats.avg()
                );
                self.last_seen_num_of_els = self.num_of_els;
                self.time_of_last_check_of_els = Instant::now();
            }
        }
    }
}

/// Counter for the min/max/avg values in the `MetricsCounter`.
#[derive(Debug)]
struct StatsCounter {
    min_value: f64,
    max_value: f64,
    samples_sum: f64,
    last_samples: VecDeque<f64>,
}

impl StatsCounter {
    const AVG_SAMPLES: usize = 20;

    pub fn new() -> Self {
        Self {
            min_value: std::f64::MAX,
            max_value: std::f64::MIN,
            samples_sum: 0.0f64,
            last_samples: VecDeque::new(),
        }
    }

    pub fn add_sample(&mut self, sample: f64) {
        if sample < self.min_value {
            self.min_value = sample;
        }

        if sample > self.max_value {
            self.max_value = sample;
        }

        if self.last_samples.len() >= Self::AVG_SAMPLES {
            let front_value = self.last_samples.pop_front().unwrap();
            self.samples_sum -= front_value;
        }

        self.last_samples.push_back(sample);
        self.samples_sum += sample;
    }

    pub fn min(&self) -> f64 {
        self.min_value
    }

    pub fn max(&self) -> f64 {
        self.max_value
    }

    pub fn avg(&self) -> f64 {
        if !self.last_samples.is_empty() {
            self.samples_sum / self.last_samples.len() as f64
        } else {
            0.0f64
        }
    }
}
