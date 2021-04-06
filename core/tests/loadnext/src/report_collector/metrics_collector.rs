use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use crate::report::ActionType;

#[derive(Debug, Clone)]
pub struct TimeHistogram {
    /// Supported time ranges.
    ranges: Vec<(u64, u64)>,
    /// Mapping from the (lower time range) to (amount of elements)
    histogram: BTreeMap<u64, usize>,
    /// Total entries in the histogram.
    total: usize,
}

impl Default for TimeHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeHistogram {
    pub fn new() -> Self {
        // Ranges from the 0 to 1000 ms with windows of 100 ms.
        let sub_sec_ranges = (0..10).map(|window_idx| Self::window(window_idx, 100));
        // Ranges from 1 second to 20 seconds with windows of 1 second.
        let sec_ranges = (1..20).map(|window_idx| Self::window(window_idx, 1000));
        // Range for (20 sec; MAX).
        let rest_range = std::iter::once((20_000u64, u64::max_value()));

        let ranges: Vec<_> = sub_sec_ranges.chain(sec_ranges).chain(rest_range).collect();
        let mut histogram = BTreeMap::new();

        for &(start, _) in ranges.iter() {
            histogram.insert(start, 0);
        }

        Self {
            ranges,
            histogram,
            total: 0,
        }
    }

    pub fn add_metric(&mut self, time: Duration) {
        let range = self.range_for(time);

        self.histogram.entry(range).and_modify(|count| *count += 1);
        self.total += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Returns the time range for the requested distribution percentile.
    pub fn percentile(&self, percentile: u64) -> (Duration, Duration) {
        let lower_gap_float = self.total as f64 * percentile as f64 / 100.0;
        let lower_gap = lower_gap_float.round() as usize;
        debug_assert!(lower_gap <= self.total);

        let mut amount = 0;
        for (range_start, current_amount) in self.histogram.iter() {
            amount += current_amount;

            if amount >= lower_gap {
                let (range_start, range_end) = self.full_range_for(*range_start);
                return (
                    Duration::from_millis(range_start),
                    Duration::from_millis(range_end),
                );
            }
        }

        unreachable!("Range for {} percentile was not found", percentile);
    }

    /// Returns the histogram entry key for the provided duration.
    fn range_for(&self, time: Duration) -> u64 {
        let duration_millis = time.as_millis() as u64;

        self.full_range_for(duration_millis).0
    }

    /// Returns the full time range for the provided duration.
    fn full_range_for(&self, duration_millis: u64) -> (u64, u64) {
        debug_assert!(self.ranges[0].0 == 0, "Ranges don't start at 0");

        for &(range_start, range_end) in self.ranges.iter().rev() {
            if duration_millis >= range_start {
                return (range_start, range_end);
            }
        }

        // First range starts from 0, and negative ranges are prohibited.
        unreachable!("Range for duration {} was not found", duration_millis);
    }

    fn window(window_idx: u64, window_size: u64) -> (u64, u64) {
        let start = window_idx * window_size;
        let end = start + window_size - 1;

        (start, end)
    }
}

/// Collector for the execution time metrics.
///
/// It builds a distribution histogram for each type of action, thus reported results are represented
/// by a range window rather than a single concrete number.
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    pub action_stats: HashMap<ActionType, TimeHistogram>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            action_stats: ActionType::all()
                .into_iter()
                .map(|action| (action, TimeHistogram::new()))
                .collect(),
        }
    }

    pub fn add_metric(&mut self, action: ActionType, time: Duration) {
        self.action_stats
            .entry(action)
            .and_modify(|hist| hist.add_metric(time));
    }

    pub fn report(&self) {
        vlog::info!("Action: [10 percentile, 50 percentile, 90 percentile]");
        for (action, histogram) in self.action_stats.iter() {
            // Only report data that was actually gathered.
            if !histogram.is_empty() {
                vlog::info!(
                    "{:?}: [>{}ms >{}ms >{}ms]",
                    action,
                    histogram.percentile(10).0.as_millis(),
                    histogram.percentile(50).0.as_millis(),
                    histogram.percentile(90).0.as_millis(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_window_size() {
        // Vector of ((window_idx, window_size), expected_range)).
        let test_vector = [
            ((0, 100), (0, 99)),
            ((1, 100), (100, 199)),
            ((2, 1000), (2000, 2999)),
        ];

        for &((window_idx, window_size), expected_result) in test_vector.iter() {
            assert_eq!(
                TimeHistogram::window(window_idx, window_size),
                expected_result
            );
        }
    }

    /// Checks that the whole diapason of u64 is covered by histogram windows.
    #[test]
    fn histogram_ranges() {
        let histogram = TimeHistogram::new();
        // Check that we start at 0 and end at max.
        assert_eq!(histogram.ranges[0].0, 0);
        assert_eq!(histogram.ranges.last().unwrap().1, u64::max_value());

        // Check that we go through all the range without gaps.
        for idx in 0..(histogram.ranges.len() - 1) {
            assert_eq!(histogram.ranges[idx].1, histogram.ranges[idx + 1].0 - 1);
        }
    }

    #[test]
    fn histogram_item_addition() {
        let mut histogram = TimeHistogram::new();

        let (first_range_start, first_range_end) = histogram.ranges[0];
        let (second_range_start, _) = histogram.ranges[1];
        let (last_range_start, last_range_end) = *histogram.ranges.last().unwrap();

        histogram.add_metric(Duration::from_millis(first_range_start));
        histogram.add_metric(Duration::from_millis(first_range_end));
        histogram.add_metric(Duration::from_millis(second_range_start));
        histogram.add_metric(Duration::from_millis(last_range_end));

        assert_eq!(histogram.histogram[&first_range_start], 2);
        assert_eq!(histogram.histogram[&second_range_start], 1);
        assert_eq!(histogram.histogram[&last_range_start], 1);
    }

    #[test]
    fn histogram_percentile() {
        let mut histogram = TimeHistogram::new();
        let first_range = (
            Duration::from_millis(histogram.ranges[0].0),
            Duration::from_millis(histogram.ranges[0].1),
        );
        let second_range = (
            Duration::from_millis(histogram.ranges[1].0),
            Duration::from_millis(histogram.ranges[1].1),
        );
        let third_range = (
            Duration::from_millis(histogram.ranges[2].0),
            Duration::from_millis(histogram.ranges[2].1),
        );

        histogram.add_metric(Duration::from_millis(0));
        for percentile in &[0, 10, 50, 90, 100] {
            assert_eq!(histogram.percentile(*percentile), first_range);
        }

        histogram.add_metric(second_range.0);
        for percentile in &[0, 10] {
            assert_eq!(histogram.percentile(*percentile), first_range);
        }
        for percentile in &[90, 100] {
            assert_eq!(histogram.percentile(*percentile), second_range);
        }

        histogram.add_metric(third_range.0);
        assert_eq!(histogram.percentile(0), first_range);
        assert_eq!(histogram.percentile(50), second_range);
        assert_eq!(histogram.percentile(100), third_range);
    }
}
