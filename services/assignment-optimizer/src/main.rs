mod config;
mod engine;
mod grpc_clients;
mod grpc_server;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use tracing::info;

use router_flow_proto::assignment::assignment_service_server::AssignmentServiceServer;

use config::Config;
use grpc_server::AssignmentServiceImpl;
use state::AppState;

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

    info!(?config, "assignment optimizer starting");

    let state = Arc::new(AppState::new(256));

    // Spawn gRPC client: consume location batches from City Simulator
    let location_state = state.clone();
    let city_sim_addr = config.city_simulator_addr.clone();
    tokio::spawn(async move {
        grpc_clients::run_location_consumer(location_state, city_sim_addr).await;
    });

    // Spawn gRPC client: consume orders from Order Generator
    let order_state = state.clone();
    let order_gen_addr = config.order_generator_addr.clone();
    tokio::spawn(async move {
        grpc_clients::run_order_consumer(order_state, order_gen_addr).await;
    });

    // Spawn the assignment engine
    let engine_state = state.clone();
    let weights = config.scoring_weights.clone();
    let interval_ms = config.assignment_interval_ms;
    let city_sim_addr = config.city_simulator_addr.clone();
    tokio::spawn(async move {
        engine::run_assignment_engine(engine_state, weights, interval_ms, city_sim_addr).await;
    });

    // Start gRPC server for WatchAssignments
    let addr: SocketAddr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    let service = AssignmentServiceImpl {
        state: state.clone(),
    };

    info!(%addr, "gRPC server listening");

    tonic::transport::Server::builder()
        .add_service(AssignmentServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
