use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Operations tracking metrics for RequestFactory batch operations.
#[derive(Debug, Default)]
pub struct OperationsTracker {
    /// Total number of operations processed
    total_operations: AtomicU64,
    /// Number of successful operations
    success_count: AtomicU64,
    /// Number of failed operations
    error_count: AtomicU64,
    /// Number of find operations
    find_count: AtomicU64,
    /// Number of persist operations
    persist_count: AtomicU64,
    /// Number of delete operations
    delete_count: AtomicU64,
    /// Total processing time in microseconds
    total_processing_time_us: AtomicU64,
    /// Recent errors (capped for memory safety)
    recent_errors: RwLock<Vec<TrackedError>>,
    /// Maximum number of recent errors to store
    max_recent_errors: usize,
}

/// A tracked error for observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedError {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub operation_type: String,
    pub entity_type: String,
    pub error: String,
}

/// Snapshot of operations metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationsMetrics {
    pub total_operations: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub find_count: u64,
    pub persist_count: u64,
    pub delete_count: u64,
    pub total_processing_time_us: u64,
    pub avg_processing_time_us: f64,
    pub success_rate: f64,
    pub recent_errors: Vec<TrackedError>,
}

impl OperationsTracker {
    pub fn new() -> Self {
        Self {
            max_recent_errors: 100,
            ..Default::default()
        }
    }

    pub fn with_max_errors(max_recent_errors: usize) -> Self {
        Self {
            max_recent_errors,
            ..Default::default()
        }
    }

    /// Record the start of an operation batch
    pub fn record_batch_start(&self, count: usize) -> BatchTimer {
        BatchTimer {
            tracker: self,
            count,
            start: std::time::Instant::now(),
        }
    }

    /// Record a successful find operation
    pub fn record_find_success(&self) {
        self.find_count.fetch_add(1, Ordering::Relaxed);
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful persist operation
    pub fn record_persist_success(&self) {
        self.persist_count.fetch_add(1, Ordering::Relaxed);
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful delete operation
    pub fn record_delete_success(&self) {
        self.delete_count.fetch_add(1, Ordering::Relaxed);
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failed operation
    pub fn record_error(&self, operation_type: &str, entity_type: &str, error: &str) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        let tracked_error = TrackedError {
            timestamp: chrono::Utc::now(),
            operation_type: operation_type.to_string(),
            entity_type: entity_type.to_string(),
            error: error.to_string(),
        };

        if let Ok(mut errors) = self.recent_errors.write() {
            errors.push(tracked_error);
            if errors.len() > self.max_recent_errors {
                errors.remove(0);
            }
        }
    }

    /// Record processing time for a batch
    pub fn record_processing_time(&self, microseconds: u64) {
        self.total_processing_time_us
            .fetch_add(microseconds, Ordering::Relaxed);
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> OperationsMetrics {
        let total = self.total_operations.load(Ordering::Relaxed);
        let success = self.success_count.load(Ordering::Relaxed);
        let errors = self.error_count.load(Ordering::Relaxed);
        let processing_time = self.total_processing_time_us.load(Ordering::Relaxed);

        OperationsMetrics {
            total_operations: total,
            success_count: success,
            error_count: errors,
            find_count: self.find_count.load(Ordering::Relaxed),
            persist_count: self.persist_count.load(Ordering::Relaxed),
            delete_count: self.delete_count.load(Ordering::Relaxed),
            total_processing_time_us: processing_time,
            avg_processing_time_us: if total > 0 {
                processing_time as f64 / total as f64
            } else {
                0.0
            },
            success_rate: if total > 0 {
                success as f64 / total as f64
            } else {
                1.0
            },
            recent_errors: self
                .recent_errors
                .read()
                .map(|g| g.clone())
                .unwrap_or_default(),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.total_operations.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.error_count.store(0, Ordering::Relaxed);
        self.find_count.store(0, Ordering::Relaxed);
        self.persist_count.store(0, Ordering::Relaxed);
        self.delete_count.store(0, Ordering::Relaxed);
        self.total_processing_time_us.store(0, Ordering::Relaxed);
        if let Ok(mut errors) = self.recent_errors.write() {
            errors.clear();
        }
    }
}

/// RAII timer for batch operations
pub struct BatchTimer<'a> {
    tracker: &'a OperationsTracker,
    count: usize,
    start: std::time::Instant,
}

impl<'a> BatchTimer<'a> {
    pub fn finish(self) -> u64 {
        let elapsed = self.start.elapsed();
        let micros = elapsed.as_micros() as u64;
        self.tracker.record_processing_time(micros);
        micros
    }
}

impl<'a> Drop for BatchTimer<'a> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let micros = elapsed.as_micros() as u64;
        self.tracker.record_processing_time(micros);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operations_tracker() {
        let tracker = OperationsTracker::new();

        tracker.record_find_success();
        tracker.record_persist_success();
        tracker.record_delete_success();
        tracker.record_error("find", "User", "not found");

        let metrics = tracker.get_metrics();
        assert_eq!(metrics.total_operations, 4);
        assert_eq!(metrics.success_count, 3);
        assert_eq!(metrics.error_count, 1);
        assert_eq!(metrics.find_count, 1);
        assert_eq!(metrics.persist_count, 1);
        assert_eq!(metrics.delete_count, 1);
    }

    #[test]
    fn test_batch_timer() {
        let tracker = OperationsTracker::new();
        {
            let _timer = tracker.record_batch_start(10);
            std::thread::sleep(std::time::Duration::from_micros(10));
        }

        let metrics = tracker.get_metrics();
        assert!(metrics.total_processing_time_us > 0);
    }

    #[test]
    fn test_reset() {
        let tracker = OperationsTracker::new();
        tracker.record_find_success();
        tracker.record_error("find", "User", "error");

        tracker.reset();

        let metrics = tracker.get_metrics();
        assert_eq!(metrics.total_operations, 0);
        assert_eq!(metrics.success_count, 0);
        assert_eq!(metrics.error_count, 0);
        assert!(metrics.recent_errors.is_empty());
    }
}
