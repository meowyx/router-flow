use std::collections::VecDeque;

use chrono::Utc;
use dashmap::DashMap;
use tokio::sync::broadcast;
use uuid::Uuid;

use router_flow_shared::models::courier::{Courier, GeoPoint};

use router_flow_proto::assignment::AssignmentEvent;

/// Lightweight courier view built from gRPC location stream
#[derive(Debug, Clone)]
pub struct CourierSnapshot {
    pub id: Uuid,
    pub position: GeoPoint,
    pub status: String,
    pub capacity: u8,
    pub current_load: u8,
    pub rating: f64,
}

impl CourierSnapshot {
    /// Convert to the shared crate's Courier type for scoring
    pub fn to_courier(&self) -> Courier {
        let status = match self.status.as_str() {
            "idle" => router_flow_shared::models::courier::CourierStatus::Available,
            _ => router_flow_shared::models::courier::CourierStatus::Busy,
        };

        Courier {
            id: self.id,
            name: String::new(),
            location: self.position.clone(),
            capacity: self.capacity,
            current_load: self.current_load,
            status,
            rating: self.rating,
            updated_at: Utc::now(),
        }
    }
}

/// A pending order waiting to be assigned
#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub order_id: String,
    pub pickup: GeoPoint,
    pub dropoff: GeoPoint,
    pub priority: String,
    pub created_at_ms: i64,
}

/// Shared state for the optimizer
pub struct AppState {
    /// Latest courier positions from City Simulator
    pub couriers: DashMap<Uuid, CourierSnapshot>,
    /// Orders waiting for assignment
    pub pending_orders: tokio::sync::Mutex<VecDeque<PendingOrder>>,
    /// Broadcast channel for assignment events (consumed by WatchAssignments)
    pub assignment_tx: broadcast::Sender<AssignmentEvent>,
}

impl AppState {
    pub fn new(event_buffer_size: usize) -> Self {
        let (assignment_tx, _) = broadcast::channel(event_buffer_size);

        Self {
            couriers: DashMap::new(),
            pending_orders: tokio::sync::Mutex::new(VecDeque::new()),
            assignment_tx,
        }
    }
}
