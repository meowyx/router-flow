use router_flow_shared::models::courier::GeoPoint;
use uuid::Uuid;

/// What a courier is currently doing.
/// Fields like `pickup` in EnRoutePickup and `order_id`/`dropoff` in EnRouteDropoff
/// are retained for state completeness even if not yet read in all code paths.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum MovementState {
    /// Wandering randomly
    Idle,
    /// Moving toward pickup location
    EnRoutePickup {
        order_id: String,
        pickup: GeoPoint,
        dropoff: GeoPoint,
    },
    /// Picked up, moving toward dropoff
    EnRouteDropoff { order_id: String, dropoff: GeoPoint },
}

/// A courier with movement state for the simulation
#[derive(Debug, Clone)]
pub struct SimCourier {
    pub id: Uuid,
    pub position: GeoPoint,
    pub waypoint: GeoPoint,
    pub state: MovementState,
    pub capacity: u8,
    pub current_load: u8,
    pub rating: f64,
}

impl SimCourier {
    /// Returns the proto-friendly status string
    pub fn status_str(&self) -> &'static str {
        match &self.state {
            MovementState::Idle => "idle",
            MovementState::EnRoutePickup { .. } => "en_route_pickup",
            MovementState::EnRouteDropoff { .. } => "en_route_dropoff",
        }
    }
}
