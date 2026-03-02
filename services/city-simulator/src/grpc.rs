use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use router_flow_proto::location::location_service_server::LocationService;
use router_flow_proto::location::{
    AssignCourierRequest, AssignCourierResponse, CourierLocationBatch, StreamLocationsRequest,
};
use router_flow_shared::models::courier::GeoPoint;

use crate::courier::{MovementState, SimCourier};

pub struct LocationServiceImpl {
    pub couriers: Arc<RwLock<Vec<SimCourier>>>,
    pub batch_tx: broadcast::Sender<CourierLocationBatch>,
}

#[tonic::async_trait]
impl LocationService for LocationServiceImpl {
    type StreamLocationsStream =
        std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<CourierLocationBatch, Status>> + Send>>;

    async fn stream_locations(
        &self,
        _request: Request<StreamLocationsRequest>,
    ) -> Result<Response<Self::StreamLocationsStream>, Status> {
        info!("new client subscribed to location stream");

        let rx = self.batch_tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(batch) => Some(Ok(batch)),
            Err(e) => {
                warn!(error = %e, "broadcast lag — client missed some ticks");
                None
            }
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn assign_courier(
        &self,
        request: Request<AssignCourierRequest>,
    ) -> Result<Response<AssignCourierResponse>, Status> {
        let req = request.into_inner();
        let courier_id = &req.courier_id;

        let pickup = req
            .pickup
            .ok_or_else(|| Status::invalid_argument("pickup location required"))?;
        let dropoff = req
            .dropoff
            .ok_or_else(|| Status::invalid_argument("dropoff location required"))?;

        let mut couriers = self.couriers.write().await;
        let courier = couriers
            .iter_mut()
            .find(|c| c.id.to_string() == *courier_id);

        match courier {
            Some(c) => {
                let pickup_geo = GeoPoint {
                    lat: pickup.latitude,
                    lng: pickup.longitude,
                };
                let dropoff_geo = GeoPoint {
                    lat: dropoff.latitude,
                    lng: dropoff.longitude,
                };

                c.waypoint = pickup_geo.clone();
                c.state = MovementState::EnRoutePickup {
                    order_id: req.order_id.clone(),
                    pickup: pickup_geo,
                    dropoff: dropoff_geo,
                };

                info!(
                    courier_id = %courier_id,
                    order_id = %req.order_id,
                    "courier assigned — en route to pickup"
                );

                Ok(Response::new(AssignCourierResponse { accepted: true }))
            }
            None => {
                warn!(courier_id = %courier_id, "courier not found");
                Ok(Response::new(AssignCourierResponse { accepted: false }))
            }
        }
    }
}
