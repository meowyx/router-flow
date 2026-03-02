use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use router_flow_proto::order::order_service_server::OrderService;
use router_flow_proto::order::{NewOrder, StreamOrdersRequest};

pub struct OrderServiceImpl {
    pub order_tx: broadcast::Sender<NewOrder>,
}

#[tonic::async_trait]
impl OrderService for OrderServiceImpl {
    type StreamOrdersStream =
        std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<NewOrder, Status>> + Send>>;

    async fn stream_orders(
        &self,
        _request: Request<StreamOrdersRequest>,
    ) -> Result<Response<Self::StreamOrdersStream>, Status> {
        info!("new client subscribed to order stream");

        let rx = self.order_tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(order) => Some(Ok(order)),
            Err(e) => {
                warn!(error = %e, "broadcast lag — client missed some orders");
                None
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }
}
