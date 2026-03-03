use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{error, warn};

use router_flow_proto::assignment::assignment_service_client::AssignmentServiceClient;
use router_flow_proto::assignment::WatchAssignmentsRequest;
use router_flow_proto::collector::collector_service_client::CollectorServiceClient;
use router_flow_proto::collector::{GetMetricsRequest, WatchCollectorEventsRequest};
use router_flow_proto::location::location_service_client::LocationServiceClient;
use router_flow_proto::location::StreamLocationsRequest;
use router_flow_proto::order::order_service_client::OrderServiceClient;
use router_flow_proto::order::StreamOrdersRequest;

/// Events funneled from gRPC streams into the TUI main loop.
#[derive(Debug)]
pub enum TuiEvent {
    /// A batch of courier locations from City Simulator.
    LocationBatch {
        total: u32,
        idle: u32,
        en_route: u32,
    },
    /// City Simulator connection state changed.
    CitySimConnected(bool),

    /// A new order from Order Generator.
    NewOrder {
        order_id: String,
        priority: String,
    },
    /// Order Generator connection state changed.
    OrderGenConnected(bool),

    /// An assignment from Assignment Optimizer.
    Assignment {
        order_id: String,
        courier_id: String,
        score: f64,
    },
    /// Optimizer connection state changed.
    OptimizerConnected(bool),

    /// A collector event (human-readable summary).
    CollectorEvent {
        summary: String,
    },
    /// Updated metrics from the Event Collector.
    CollectorMetrics {
        total_assignments: i64,
        total_events_processed: i64,
        avg_latency_ms: f64,
        p95_latency_ms: f64,
        courier_utilization_pct: f64,
        avg_score: f64,
        uptime_seconds: i64,
    },
    /// Collector connection state changed.
    CollectorConnected(bool),
}

/// Subscribe to City Simulator's StreamLocations. Reconnects on failure.
pub async fn run_location_stream(tx: mpsc::Sender<TuiEvent>, addr: String) {
    loop {
        match LocationServiceClient::connect(addr.clone()).await {
            Ok(mut client) => {
                let _ = tx.send(TuiEvent::CitySimConnected(true)).await;

                match client.stream_locations(StreamLocationsRequest {}).await {
                    Ok(response) => {
                        let mut stream = response.into_inner();
                        loop {
                            match stream.message().await {
                                Ok(Some(batch)) => {
                                    let total = batch.locations.len() as u32;
                                    let idle = batch
                                        .locations
                                        .iter()
                                        .filter(|l| l.status == "idle")
                                        .count()
                                        as u32;
                                    let en_route = total - idle;

                                    let _ = tx
                                        .send(TuiEvent::LocationBatch {
                                            total,
                                            idle,
                                            en_route,
                                        })
                                        .await;
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    warn!(error = %e, "city-sim stream error");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => error!(error = %e, "failed to start location stream"),
                }
            }
            Err(e) => error!(error = %e, "failed to connect to city-sim"),
        }

        let _ = tx.send(TuiEvent::CitySimConnected(false)).await;
        sleep(Duration::from_secs(2)).await;
    }
}

/// Subscribe to Order Generator's StreamOrders. Reconnects on failure.
pub async fn run_order_stream(tx: mpsc::Sender<TuiEvent>, addr: String) {
    loop {
        match OrderServiceClient::connect(addr.clone()).await {
            Ok(mut client) => {
                let _ = tx.send(TuiEvent::OrderGenConnected(true)).await;

                match client.stream_orders(StreamOrdersRequest {}).await {
                    Ok(response) => {
                        let mut stream = response.into_inner();
                        loop {
                            match stream.message().await {
                                Ok(Some(order)) => {
                                    let _ = tx
                                        .send(TuiEvent::NewOrder {
                                            order_id: order.order_id,
                                            priority: order.priority,
                                        })
                                        .await;
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    warn!(error = %e, "order-gen stream error");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => error!(error = %e, "failed to start order stream"),
                }
            }
            Err(e) => error!(error = %e, "failed to connect to order-gen"),
        }

        let _ = tx.send(TuiEvent::OrderGenConnected(false)).await;
        sleep(Duration::from_secs(2)).await;
    }
}

/// Subscribe to Assignment Optimizer's WatchAssignments. Reconnects on failure.
pub async fn run_assignment_stream(tx: mpsc::Sender<TuiEvent>, addr: String) {
    loop {
        match AssignmentServiceClient::connect(addr.clone()).await {
            Ok(mut client) => {
                let _ = tx.send(TuiEvent::OptimizerConnected(true)).await;

                match client
                    .watch_assignments(WatchAssignmentsRequest {})
                    .await
                {
                    Ok(response) => {
                        let mut stream = response.into_inner();
                        loop {
                            match stream.message().await {
                                Ok(Some(event)) => {
                                    let _ = tx
                                        .send(TuiEvent::Assignment {
                                            order_id: event.order_id,
                                            courier_id: event.courier_id,
                                            score: event.score,
                                        })
                                        .await;
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    warn!(error = %e, "optimizer stream error");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => error!(error = %e, "failed to start assignment stream"),
                }
            }
            Err(e) => error!(error = %e, "failed to connect to optimizer"),
        }

        let _ = tx.send(TuiEvent::OptimizerConnected(false)).await;
        sleep(Duration::from_secs(2)).await;
    }
}

/// Subscribe to Event Collector's WatchCollectorEvents. Reconnects on failure.
pub async fn run_collector_event_stream(tx: mpsc::Sender<TuiEvent>, addr: String) {
    loop {
        match CollectorServiceClient::connect(addr.clone()).await {
            Ok(mut client) => {
                let _ = tx.send(TuiEvent::CollectorConnected(true)).await;

                match client
                    .watch_collector_events(WatchCollectorEventsRequest {})
                    .await
                {
                    Ok(response) => {
                        let mut stream = response.into_inner();
                        loop {
                            match stream.message().await {
                                Ok(Some(event)) => {
                                    let _ = tx
                                        .send(TuiEvent::CollectorEvent {
                                            summary: event.summary,
                                        })
                                        .await;
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    warn!(error = %e, "collector event stream error");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => error!(error = %e, "failed to start collector event stream"),
                }
            }
            Err(e) => error!(error = %e, "failed to connect to collector"),
        }

        let _ = tx.send(TuiEvent::CollectorConnected(false)).await;
        sleep(Duration::from_secs(2)).await;
    }
}

/// Poll Event Collector's GetMetrics RPC periodically.
pub async fn run_metrics_poller(tx: mpsc::Sender<TuiEvent>, addr: String) {
    loop {
        match CollectorServiceClient::connect(addr.clone()).await {
            Ok(mut client) => loop {
                match client.get_metrics(GetMetricsRequest {}).await {
                    Ok(response) => {
                        let m = response.into_inner();
                        let _ = tx
                            .send(TuiEvent::CollectorMetrics {
                                total_assignments: m.total_assignments,
                                total_events_processed: m.total_events_processed,
                                avg_latency_ms: m.avg_assignment_latency_ms,
                                p95_latency_ms: m.p95_assignment_latency_ms,
                                courier_utilization_pct: m.courier_utilization_pct,
                                avg_score: m.avg_score,
                                uptime_seconds: m.uptime_seconds,
                            })
                            .await;
                    }
                    Err(e) => {
                        warn!(error = %e, "metrics poll failed");
                        break;
                    }
                }
                sleep(Duration::from_secs(2)).await;
            },
            Err(_) => {}
        }

        sleep(Duration::from_secs(3)).await;
    }
}
