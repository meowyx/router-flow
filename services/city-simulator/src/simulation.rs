use std::sync::Arc;

use chrono::Utc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, info};

use router_flow_proto::location::{CourierLocation, CourierLocationBatch, GeoPoint as ProtoGeoPoint};

use crate::config::Config;
use crate::courier::SimCourier;

/// Runs the simulation tick loop.
/// Each tick: moves all couriers, builds a batch, and broadcasts it.
pub async fn run_simulation(
    config: Config,
    couriers: Arc<RwLock<Vec<SimCourier>>>,
    batch_tx: broadcast::Sender<CourierLocationBatch>,
) {
    let mut tick = interval(Duration::from_millis(config.tick_interval_ms));
    let mut tick_count: u64 = 0;

    info!(
        num_couriers = config.num_couriers,
        tick_ms = config.tick_interval_ms,
        speed = config.movement_speed,
        "simulation started"
    );

    loop {
        tick.tick().await;
        tick_count += 1;

        let batch = {
            let mut couriers = couriers.write().await;
            let mut locations = Vec::with_capacity(couriers.len());

            for courier in couriers.iter_mut() {
                let reached = courier.step(config.movement_speed);
                if reached {
                    courier.on_waypoint_reached(&config);
                }

                locations.push(CourierLocation {
                    courier_id: courier.id.to_string(),
                    position: Some(ProtoGeoPoint {
                        latitude: courier.position.lat,
                        longitude: courier.position.lng,
                    }),
                    status: courier.status_str().to_string(),
                    capacity: courier.capacity as u32,
                    current_load: courier.current_load as u32,
                    rating: courier.rating,
                    timestamp_ms: Utc::now().timestamp_millis(),
                });
            }

            CourierLocationBatch {
                locations,
                tick_ms: Utc::now().timestamp_millis(),
            }
        };

        // Log every 20 ticks to avoid spam
        if tick_count % 20 == 0 {
            debug!(tick = tick_count, couriers = batch.locations.len(), "tick");
        }

        // If no subscribers, that's fine — just drop the batch
        let _ = batch_tx.send(batch);
    }
}
