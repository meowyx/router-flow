use std::sync::Arc;

use chrono::Utc;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};
use uuid::Uuid;

use router_flow_proto::assignment::{AssignmentEvent, ScoreBreakdown as ProtoScoreBreakdown};
use router_flow_proto::location::location_service_client::LocationServiceClient;
use router_flow_proto::location::{AssignCourierRequest, GeoPoint as ProtoGeoPoint};

use router_flow_shared::engine::scoring::{compute_score, ScoringWeights};
use router_flow_shared::models::courier::{Courier, CourierStatus, GeoPoint};
use router_flow_shared::models::order::{DeliveryOrder, OrderStatus, Priority};

use crate::state::{AppState, PendingOrder};

/// Runs the assignment loop on a fixed interval.
/// Each tick: drains pending orders, scores available couriers, makes assignments.
pub async fn run_assignment_engine(
    state: Arc<AppState>,
    weights: ScoringWeights,
    interval_ms: u64,
    city_sim_addr: String,
) {
    let mut tick = interval(Duration::from_millis(interval_ms));

    info!(interval_ms = interval_ms, "assignment engine started");

    loop {
        tick.tick().await;

        // Drain all pending orders for this batch
        let orders: Vec<PendingOrder> = {
            let mut queue = state.pending_orders.lock().await;
            queue.drain(..).collect()
        };

        if orders.is_empty() {
            continue;
        }

        // Get available couriers snapshot
        let available: Vec<Courier> = state
            .couriers
            .iter()
            .filter(|entry| entry.value().status == "idle")
            .map(|entry| entry.value().to_courier())
            .collect();

        if available.is_empty() {
            // Re-queue all orders
            let mut queue = state.pending_orders.lock().await;
            for order in orders {
                queue.push_back(order);
            }
            debug!(requeued = queue.len(), "no available couriers");
            continue;
        }

        debug!(
            orders = orders.len(),
            couriers = available.len(),
            "running assignment batch"
        );

        // Track which couriers have been assigned this batch
        let mut assigned_couriers: Vec<Uuid> = Vec::new();

        for pending in &orders {
            let order = to_delivery_order(pending);

            // Filter out couriers already assigned in this batch
            let candidates: Vec<&Courier> = available
                .iter()
                .filter(|c| {
                    c.status == CourierStatus::Available
                        && c.current_load < c.capacity
                        && !assigned_couriers.contains(&c.id)
                })
                .collect();

            if candidates.is_empty() {
                // Re-queue this order
                let mut queue = state.pending_orders.lock().await;
                queue.push_back(pending.clone());
                continue;
            }

            // Score all candidates and pick the best
            let best = candidates
                .iter()
                .map(|c| {
                    let (score, breakdown) = compute_score(c, &order, &weights);
                    (*c, score, breakdown)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1));

            let (winner, score, breakdown) = match best {
                Some(b) => b,
                None => continue,
            };

            assigned_couriers.push(winner.id);

            // Notify City Simulator to move courier to pickup
            let addr = city_sim_addr.clone();
            let courier_id = winner.id.to_string();
            let order_id = pending.order_id.clone();
            let pickup = pending.pickup.clone();
            let dropoff = pending.dropoff.clone();

            tokio::spawn(async move {
                notify_city_simulator(addr, courier_id, order_id, pickup, dropoff).await;
            });

            // Broadcast assignment event
            let event = AssignmentEvent {
                assignment_id: Uuid::new_v4().to_string(),
                order_id: pending.order_id.clone(),
                courier_id: winner.id.to_string(),
                score,
                score_breakdown: Some(ProtoScoreBreakdown {
                    distance_score: breakdown.distance_score,
                    load_score: breakdown.load_score,
                    rating_score: breakdown.rating_score,
                    priority_score: breakdown.priority_score,
                }),
                assigned_at_ms: Utc::now().timestamp_millis(),
                order_created_at_ms: pending.created_at_ms,
            };

            info!(
                order_id = %pending.order_id,
                courier_id = %winner.id,
                score = score,
                "order assigned"
            );

            let _ = state.assignment_tx.send(event);
        }
    }
}

/// Convert a PendingOrder to the shared crate's DeliveryOrder for scoring
fn to_delivery_order(pending: &PendingOrder) -> DeliveryOrder {
    let priority = match pending.priority.as_str() {
        "low" => Priority::Low,
        "high" => Priority::High,
        "urgent" => Priority::Urgent,
        _ => Priority::Normal,
    };

    DeliveryOrder {
        id: pending.order_id.parse().unwrap_or_else(|_| Uuid::new_v4()),
        pickup: pending.pickup.clone(),
        dropoff: pending.dropoff.clone(),
        priority,
        status: OrderStatus::Pending,
        assigned_courier: None,
        created_at: chrono::DateTime::from_timestamp_millis(pending.created_at_ms)
            .unwrap_or_else(|| Utc::now()),
    }
}

/// Call City Simulator's AssignCourier RPC
async fn notify_city_simulator(
    addr: String,
    courier_id: String,
    order_id: String,
    pickup: GeoPoint,
    dropoff: GeoPoint,
) {
    match LocationServiceClient::connect(addr).await {
        Ok(mut client) => {
            let request = AssignCourierRequest {
                courier_id: courier_id.clone(),
                order_id: order_id.clone(),
                pickup: Some(ProtoGeoPoint {
                    latitude: pickup.lat,
                    longitude: pickup.lng,
                }),
                dropoff: Some(ProtoGeoPoint {
                    latitude: dropoff.lat,
                    longitude: dropoff.lng,
                }),
            };

            match client.assign_courier(request).await {
                Ok(response) => {
                    if response.into_inner().accepted {
                        debug!(courier_id = %courier_id, order_id = %order_id, "assignment accepted by city simulator");
                    } else {
                        warn!(courier_id = %courier_id, "assignment rejected by city simulator");
                    }
                }
                Err(e) => {
                    warn!(error = %e, "failed to notify city simulator of assignment");
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "failed to connect to city simulator for assignment");
        }
    }
}
