use std::sync::Arc;

use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use router_flow_proto::assignment::assignment_service_server::AssignmentService;
use router_flow_proto::assignment::{AssignmentEvent, WatchAssignmentsRequest};

use crate::state::AppState;

pub struct AssignmentServiceImpl {
    pub state: Arc<AppState>,
}

#[tonic::async_trait]
impl AssignmentService for AssignmentServiceImpl {
    type WatchAssignmentsStream =
        std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<AssignmentEvent, Status>> + Send>>;

    async fn watch_assignments(
        &self,
        _request: Request<WatchAssignmentsRequest>,
    ) -> Result<Response<Self::WatchAssignmentsStream>, Status> {
        info!("new client subscribed to assignment stream");

        let rx = self.state.assignment_tx.subscribe();
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
