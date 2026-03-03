use std::collections::{HashMap, VecDeque};

use chrono::Utc;

/// A single recorded assignment for metric aggregation.
#[derive(Debug, Clone)]
pub struct AssignmentRecord {
    pub courier_id: String,
    pub score: f64,
    /// Latency in milliseconds from order creation to assignment.
    pub latency_ms: i64,
    /// Timestamp when the assignment was recorded (epoch ms).
    pub recorded_at_ms: i64,
}

/// Sliding-window metric aggregator.
///
/// Keeps a window of recent assignment records and computes:
/// - Total assignments (all-time)
/// - Average assignment latency (windowed)
/// - p95/p99 assignment latency (windowed)
/// - Average score (windowed)
/// - Per-courier assignment counts (for utilization)
pub struct Aggregator {
    /// All-time total assignments count.
    total_assignments: u64,
    /// All-time total events processed (including future event types).
    total_events_processed: u64,
    /// Sliding window of recent records.
    window: VecDeque<AssignmentRecord>,
    /// Window size in milliseconds.
    window_ms: i64,
    /// Per-courier assignment counts (all-time).
    courier_assignment_counts: HashMap<String, u64>,
}

impl Aggregator {
    pub fn new(window_size_secs: u64) -> Self {
        Self {
            total_assignments: 0,
            total_events_processed: 0,
            window: VecDeque::new(),
            window_ms: (window_size_secs * 1000) as i64,
            courier_assignment_counts: HashMap::new(),
        }
    }

    /// Record a new assignment event.
    pub fn record_assignment(&mut self, courier_id: String, score: f64, latency_ms: i64) {
        let now_ms = Utc::now().timestamp_millis();

        self.total_assignments += 1;
        self.total_events_processed += 1;

        *self
            .courier_assignment_counts
            .entry(courier_id.clone())
            .or_insert(0) += 1;

        self.window.push_back(AssignmentRecord {
            courier_id,
            score,
            latency_ms,
            recorded_at_ms: now_ms,
        });

        self.evict_expired(now_ms);
    }

    /// Remove records outside the sliding window.
    fn evict_expired(&mut self, now_ms: i64) {
        let cutoff = now_ms - self.window_ms;
        while let Some(front) = self.window.front() {
            if front.recorded_at_ms < cutoff {
                self.window.pop_front();
            } else {
                break;
            }
        }
    }

    /// Total assignments (all-time).
    pub fn total_assignments(&self) -> u64 {
        self.total_assignments
    }

    /// Total events processed (all-time).
    pub fn total_events_processed(&self) -> u64 {
        self.total_events_processed
    }

    /// Average assignment latency in ms (windowed).
    pub fn avg_latency_ms(&mut self) -> f64 {
        self.evict_expired(Utc::now().timestamp_millis());

        if self.window.is_empty() {
            return 0.0;
        }

        let sum: i64 = self.window.iter().map(|r| r.latency_ms).sum();
        sum as f64 / self.window.len() as f64
    }

    /// Compute a latency percentile (windowed). `pct` should be 0.0..1.0.
    pub fn latency_percentile(&mut self, pct: f64) -> f64 {
        self.evict_expired(Utc::now().timestamp_millis());

        if self.window.is_empty() {
            return 0.0;
        }

        let mut latencies: Vec<i64> = self.window.iter().map(|r| r.latency_ms).collect();
        latencies.sort_unstable();

        let idx = ((pct * latencies.len() as f64) - 1.0)
            .max(0.0)
            .min((latencies.len() - 1) as f64) as usize;

        latencies[idx] as f64
    }

    /// Average score (windowed).
    pub fn avg_score(&mut self) -> f64 {
        self.evict_expired(Utc::now().timestamp_millis());

        if self.window.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.window.iter().map(|r| r.score).sum();
        sum / self.window.len() as f64
    }

    /// Number of distinct couriers that received assignments (all-time).
    pub fn active_couriers(&self) -> usize {
        self.courier_assignment_counts.len()
    }

    /// Courier utilization: percentage of couriers with >0 assignments in the window.
    /// Based on window data relative to known couriers.
    pub fn courier_utilization_pct(&mut self, total_couriers: usize) -> f64 {
        self.evict_expired(Utc::now().timestamp_millis());

        if total_couriers == 0 {
            return 0.0;
        }

        let mut active_in_window: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for record in &self.window {
            active_in_window.insert(&record.courier_id);
        }

        (active_in_window.len() as f64 / total_couriers as f64) * 100.0
    }

    /// Current window size (for testing).
    #[cfg(test)]
    pub fn window_len(&self) -> usize {
        self.window.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_aggregator_returns_zeroes() {
        let mut agg = Aggregator::new(60);
        assert_eq!(agg.total_assignments(), 0);
        assert_eq!(agg.total_events_processed(), 0);
        assert!((agg.avg_latency_ms() - 0.0).abs() < f64::EPSILON);
        assert!((agg.latency_percentile(0.95) - 0.0).abs() < f64::EPSILON);
        assert!((agg.avg_score() - 0.0).abs() < f64::EPSILON);
        assert_eq!(agg.active_couriers(), 0);
    }

    #[test]
    fn single_assignment_metrics() {
        let mut agg = Aggregator::new(60);
        agg.record_assignment("courier-1".to_string(), 0.85, 120);

        assert_eq!(agg.total_assignments(), 1);
        assert!((agg.avg_latency_ms() - 120.0).abs() < f64::EPSILON);
        assert!((agg.latency_percentile(0.95) - 120.0).abs() < f64::EPSILON);
        assert!((agg.avg_score() - 0.85).abs() < f64::EPSILON);
        assert_eq!(agg.active_couriers(), 1);
    }

    #[test]
    fn percentile_with_multiple_records() {
        let mut agg = Aggregator::new(60);

        // 10 records with latencies 10, 20, 30, ..., 100
        for i in 1..=10 {
            agg.record_assignment(format!("courier-{}", i % 3), 0.5, i * 10);
        }

        assert_eq!(agg.total_assignments(), 10);

        let p95 = agg.latency_percentile(0.95);
        // 95th percentile of [10,20,30,40,50,60,70,80,90,100] = index 8 = 90
        assert!((p95 - 90.0).abs() < f64::EPSILON);

        let p50 = agg.latency_percentile(0.50);
        // 50th percentile = index 4 = 50
        assert!((p50 - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn avg_score_computed_correctly() {
        let mut agg = Aggregator::new(60);
        agg.record_assignment("c1".to_string(), 0.80, 10);
        agg.record_assignment("c2".to_string(), 0.60, 20);
        agg.record_assignment("c3".to_string(), 1.00, 30);

        let expected = (0.80 + 0.60 + 1.00) / 3.0;
        assert!((agg.avg_score() - expected).abs() < 1e-10);
    }

    #[test]
    fn utilization_tracks_unique_couriers_in_window() {
        let mut agg = Aggregator::new(60);
        agg.record_assignment("c1".to_string(), 0.5, 10);
        agg.record_assignment("c1".to_string(), 0.6, 20); // same courier
        agg.record_assignment("c2".to_string(), 0.7, 30);

        // 2 unique couriers active out of 5 total
        let util = agg.courier_utilization_pct(5);
        assert!((util - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn window_eviction_works() {
        let mut agg = Aggregator::new(60);

        // Manually push an old record
        agg.total_assignments += 1;
        agg.total_events_processed += 1;
        agg.window.push_back(AssignmentRecord {
            courier_id: "old".to_string(),
            score: 0.5,
            latency_ms: 999,
            recorded_at_ms: Utc::now().timestamp_millis() - 120_000, // 2 min ago
        });

        // Record a fresh one
        agg.record_assignment("new".to_string(), 0.8, 50);

        // The old record should be evicted; window should only have the new one
        assert_eq!(agg.window_len(), 1);
        assert!((agg.avg_latency_ms() - 50.0).abs() < f64::EPSILON);
        // But total is still 2 (all-time)
        assert_eq!(agg.total_assignments(), 2);
    }
}
