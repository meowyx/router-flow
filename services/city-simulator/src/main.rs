mod config;
mod courier;
mod grpc;
mod movement;
mod simulation;

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};
use tracing::info;

use router_flow_proto::location::location_service_server::LocationServiceServer;
use router_flow_proto::location::CourierLocationBatch;

use config::Config;
use courier::SimCourier;
use grpc::LocationServiceImpl;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .compact()
        .init();

    info!(?config, "city simulator starting");

    // Initialize couriers at random positions
    let couriers: Vec<SimCourier> = (0..config.num_couriers)
        .map(|_| SimCourier::new_random(&config))
        .collect();

    info!(count = couriers.len(), "couriers initialized");

    let couriers = Arc::new(RwLock::new(couriers));
    let (batch_tx, _) = broadcast::channel::<CourierLocationBatch>(128);

    // Spawn the simulation tick loop
    let sim_config = config.clone();
    let sim_couriers = couriers.clone();
    let sim_tx = batch_tx.clone();
    tokio::spawn(async move {
        simulation::run_simulation(sim_config, sim_couriers, sim_tx).await;
    });

    // Start gRPC server
    let addr: SocketAddr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    let service = LocationServiceImpl {
        couriers,
        batch_tx,
    };

    info!(%addr, "gRPC server listening");

    tonic::transport::Server::builder()
        .add_service(LocationServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
