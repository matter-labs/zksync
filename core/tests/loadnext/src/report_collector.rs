use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use futures::{channel::mpsc::Receiver, StreamExt};

use crate::report::{ActionType, Report};

#[derive(Debug, Clone)]
pub struct TimeHistogram {
    /// Supported time ranges.
    pub ranges: Vec<(u64, u64)>,
    /// Mapping from the (lower time range) to (amount of elements)
    pub histogram: BTreeMap<u64, usize>,
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

        Self { ranges, histogram }
    }

    pub fn add_metric(&mut self, duration: Duration) {
        let range = self.range_for(duration);

        self.histogram.entry(range).and_modify(|count| *count += 1);
    }

    /// Returns the histogram entry key for the provided duration.
    fn range_for(&self, duration: Duration) -> u64 {
        debug_assert!(self.ranges[0].0 == 0, "Ranges don't start at 0");

        let duration_millis = duration.as_millis() as u64;
        for &(range_start, _) in self.ranges.iter().rev() {
            if duration_millis >= range_start {
                return range_start;
            }
        }

        // First range starts from 0, and negative ranges are prohibited.
        unreachable!()
    }

    fn window(window_idx: u64, window_size: u64) -> (u64, u64) {
        let start = window_idx * window_size;
        let end = start + window_size - 1;

        (start, end)
    }
}

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
            action_stats: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct ReportCollector {
    reports_stream: Receiver<Report>,
}

impl ReportCollector {
    pub fn new(reports_stream: Receiver<Report>) -> Self {
        Self { reports_stream }
    }

    pub async fn run(mut self) {
        while let Some(report) = self.reports_stream.next().await {
            vlog::trace!("Report: {:?}", &report);

            todo!()
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
}
