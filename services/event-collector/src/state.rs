use std::sync::atomic::AtomicUsize;

use tokio::sync::broadcast;

use router_flow_proto::collector::CollectorEvent;

/// Shared state for the Event Collector.
pub struct CollectorState {
    /// Broadcast channel for collector events (consumed by WatchCollectorEvents).
    pub event_tx: broadcast::Sender<CollectorEvent>,
    /// Total number of known couriers (updated from assignment events).
    pub total_couriers: AtomicUsize,
}

impl CollectorState {
    pub fn new(event_buffer_size: usize) -> Self {
        let (event_tx, _) = broadcast::channel(event_buffer_size);

        Self {
            event_tx,
            total_couriers: AtomicUsize::new(25), // default, updated as we see courier IDs
        }
    }
}
