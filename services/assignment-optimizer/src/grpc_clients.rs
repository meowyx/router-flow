use std::sync::Arc;

use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

use router_flow_proto::location::location_service_client::LocationServiceClient;
use router_flow_proto::location::StreamLocationsRequest;
use router_flow_proto::order::order_service_client::OrderServiceClient;
use router_flow_proto::order::StreamOrdersRequest;

use router_flow_shared::models::courier::GeoPoint;

use crate::state::{AppState, CourierSnapshot, PendingOrder};

/// Connect to City Simulator and consume location batches.
/// Updates courier snapshots in state. Reconnects on failure.
pub async fn run_location_consumer(state: Arc<AppState>, addr: String) {
    loop {
        info!(addr = %addr, "connecting to city simulator");

        match LocationServiceClient::connect(addr.clone()).await {
            Ok(mut client) => {
                info!("connected to city simulator");

                match client.stream_locations(StreamLocationsRequest {}).await {
                    Ok(response) => {
                        let mut stream = response.into_inner();

                        loop {
                            match stream.message().await {
                                Ok(Some(batch)) => {
                                    for loc in &batch.locations {
                                        let id = match loc.courier_id.parse::<Uuid>() {
                                            Ok(id) => id,
                                            Err(_) => continue,
                                        };

                                        let position = match &loc.position {
                                            Some(p) => GeoPoint {
                                                lat: p.latitude,
                                                lng: p.longitude,
                                            },
                                            None => continue,
                                        };

                                        state.couriers.insert(
                                            id,
                                            CourierSnapshot {
                                                id,
                                                position,
                                                status: loc.status.clone(),
                                                capacity: loc.capacity as u8,
                                                current_load: loc.current_load as u8,
                                                rating: loc.rating,
                                            },
                                        );
                                    }
                                }
                                Ok(None) => {
                                    warn!("city simulator stream ended");
                                    break;
                                }
                                Err(e) => {
                                    error!(error = %e, "city simulator stream error");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "failed to start location stream");
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "failed to connect to city simulator");
            }
        }

        warn!("reconnecting to city simulator in 2s");
        sleep(Duration::from_secs(2)).await;
    }
}

/// Connect to Order Generator and consume new orders.
/// Pushes orders into the pending queue. Reconnects on failure.
pub async fn run_order_consumer(state: Arc<AppState>, addr: String) {
    loop {
        info!(addr = %addr, "connecting to order generator");

        match OrderServiceClient::connect(addr.clone()).await {
            Ok(mut client) => {
                info!("connected to order generator");

                match client.stream_orders(StreamOrdersRequest {}).await {
                    Ok(response) => {
                        let mut stream = response.into_inner();

                        loop {
                            match stream.message().await {
                                Ok(Some(order)) => {
                                    let pickup = match &order.pickup {
                                        Some(p) => GeoPoint {
                                            lat: p.latitude,
                                            lng: p.longitude,
                                        },
                                        None => continue,
                                    };
                                    let dropoff = match &order.dropoff {
                                        Some(d) => GeoPoint {
                                            lat: d.latitude,
                                            lng: d.longitude,
                                        },
                                        None => continue,
                                    };

                                    let pending = PendingOrder {
                                        order_id: order.order_id,
                                        pickup,
                                        dropoff,
                                        priority: order.priority,
                                        created_at_ms: order.created_at_ms,
                                    };

                                    let mut queue = state.pending_orders.lock().await;
                                    queue.push_back(pending);
                                }
                                Ok(None) => {
                                    warn!("order generator stream ended");
                                    break;
                                }
                                Err(e) => {
                                    error!(error = %e, "order generator stream error");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "failed to start order stream");
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "failed to connect to order generator");
            }
        }

        warn!("reconnecting to order generator in 2s");
        sleep(Duration::from_secs(2)).await;
    }
}
