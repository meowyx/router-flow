use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use router_flow_proto::collector::collector_service_server::CollectorService;
use router_flow_proto::collector::{
    CollectorEvent, GetMetricsRequest, GetMetricsResponse, WatchCollectorEventsRequest,
};

use crate::aggregator::Aggregator;
use crate::state::CollectorState;

pub struct CollectorServiceImpl {
    pub state: Arc<CollectorState>,
    pub aggregator: Arc<Mutex<Aggregator>>,
    pub start_time: std::time::Instant,
}

#[tonic::async_trait]
impl CollectorService for CollectorServiceImpl {
    async fn get_metrics(
        &self,
        _request: Request<GetMetricsRequest>,
    ) -> Result<Response<GetMetricsResponse>, Status> {
        let mut agg = self.aggregator.lock().await;
        let total_couriers = self
            .state
            .total_couriers
            .load(Ordering::Relaxed);

        let response = GetMetricsResponse {
            total_assignments: agg.total_assignments() as i64,
            total_events_processed: agg.total_events_processed() as i64,
            avg_assignment_latency_ms: agg.avg_latency_ms(),
            p95_assignment_latency_ms: agg.latency_percentile(0.95),
            p99_assignment_latency_ms: agg.latency_percentile(0.99),
            courier_utilization_pct: agg.courier_utilization_pct(total_couriers),
            active_couriers: agg.active_couriers() as i32,
            orders_in_queue: 0, // TODO: could relay from optimizer
            avg_score: agg.avg_score(),
            uptime_seconds: self.start_time.elapsed().as_secs() as i64,
        };

        Ok(Response::new(response))
    }

    type WatchCollectorEventsStream =
        std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<CollectorEvent, Status>> + Send>>;

    async fn watch_collector_events(
        &self,
        _request: Request<WatchCollectorEventsRequest>,
    ) -> Result<Response<Self::WatchCollectorEventsStream>, Status> {
        info!("new client subscribed to collector event stream");

        let rx = self.state.event_tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(event) => Some(Ok(event)),
            Err(e) => {
                warn!(error = %e, "broadcast lag — client missed some events");
                None
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }
}
