use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

use router_flow_proto::assignment::assignment_service_client::AssignmentServiceClient;
use router_flow_proto::assignment::WatchAssignmentsRequest;
use router_flow_proto::collector::CollectorEvent;

use crate::aggregator::Aggregator;
use crate::state::CollectorState;

/// Connect to the Assignment Optimizer's WatchAssignments stream.
/// Records each assignment into the aggregator and broadcasts collector events.
/// Reconnects on failure.
pub async fn run_assignment_consumer(
    state: Arc<CollectorState>,
    aggregator: Arc<Mutex<Aggregator>>,
    addr: String,
) {
    let mut known_couriers: HashSet<String> = HashSet::new();

    loop {
        info!(addr = %addr, "connecting to assignment optimizer");

        match AssignmentServiceClient::connect(addr.clone()).await {
            Ok(mut client) => {
                info!("connected to assignment optimizer");

                match client
                    .watch_assignments(WatchAssignmentsRequest {})
                    .await
                {
                    Ok(response) => {
                        let mut stream = response.into_inner();

                        loop {
                            match stream.message().await {
                                Ok(Some(event)) => {
                                    let latency_ms =
                                        event.assigned_at_ms - event.order_created_at_ms;

                                    // Update aggregator
                                    {
                                        let mut agg = aggregator.lock().await;
                                        agg.record_assignment(
                                            event.courier_id.clone(),
                                            event.score,
                                            latency_ms,
                                        );
                                    }

                                    // Track unique couriers for utilization
                                    if known_couriers.insert(event.courier_id.clone()) {
                                        state
                                            .total_couriers
                                            .store(known_couriers.len(), Ordering::Relaxed);
                                    }

                                    // Broadcast a collector event for TUI
                                    let collector_event = CollectorEvent {
                                        event_id: Uuid::new_v4().to_string(),
                                        event_type: "assignment_recorded".to_string(),
                                        summary: format!(
                                            "assigned order={} to courier={} score={:.2} latency={}ms",
                                            short_id(&event.order_id),
                                            short_id(&event.courier_id),
                                            event.score,
                                            latency_ms,
                                        ),
                                        timestamp_ms: Utc::now().timestamp_millis(),
                                    };

                                    // Ignore send error (no subscribers is fine)
                                    let _ = state.event_tx.send(collector_event);

                                    info!(
                                        order_id = %short_id(&event.order_id),
                                        courier_id = %short_id(&event.courier_id),
                                        score = event.score,
                                        latency_ms,
                                        "recorded assignment"
                                    );
                                }
                                Ok(None) => {
                                    warn!("assignment optimizer stream ended");
                                    break;
                                }
                                Err(e) => {
                                    error!(error = %e, "assignment optimizer stream error");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "failed to start assignment stream");
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "failed to connect to assignment optimizer");
            }
        }

        warn!("reconnecting to assignment optimizer in 2s");
        sleep(Duration::from_secs(2)).await;
    }
}

/// Shorten a UUID string for log readability.
fn short_id(id: &str) -> &str {
    if id.len() > 8 {
        &id[..8]
    } else {
        id
    }
}
