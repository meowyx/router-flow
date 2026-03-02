use tokio::sync::broadcast;
use tokio::time::{sleep, Duration};
use tracing::{debug, info};

use router_flow_proto::order::NewOrder;

use crate::config::Config;
use crate::patterns;

/// Order generation loop — generates orders at the configured rate and broadcasts them
pub async fn run_generator(config: Config, order_tx: broadcast::Sender<NewOrder>) {
    let interval_ms = (1000.0 / config.order_rate_per_sec) as u64;

    info!(
        rate = config.order_rate_per_sec,
        pattern = ?config.pattern,
        interval_ms = interval_ms,
        "order generator started"
    );

    let mut count: u64 = 0;

    loop {
        sleep(Duration::from_millis(interval_ms)).await;

        let order = patterns::generate_order(&config);
        count += 1;

        debug!(
            order_id = %order.order_id,
            priority = %order.priority,
            count = count,
            "order generated"
        );

        let _ = order_tx.send(order);
    }
}
