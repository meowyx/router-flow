mod aggregator;
mod config;
mod grpc_client;
mod grpc_server;
mod metrics;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use router_flow_proto::collector::collector_service_server::CollectorServiceServer;

use aggregator::Aggregator;
use config::Config;
use grpc_server::CollectorServiceImpl;
use state::CollectorState;

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

    info!(?config, "event collector starting");

    let state = Arc::new(CollectorState::new(256));
    let aggregator = Arc::new(Mutex::new(Aggregator::new(config.window_size_secs)));
    let start_time = std::time::Instant::now();

    // Spawn gRPC client: consume assignment events from Optimizer
    let client_state = state.clone();
    let client_aggregator = aggregator.clone();
    let optimizer_addr = config.optimizer_addr.clone();
    tokio::spawn(async move {
        grpc_client::run_assignment_consumer(client_state, client_aggregator, optimizer_addr).await;
    });

    // Spawn HTTP /metrics server
    let http_state = state.clone();
    let http_aggregator = aggregator.clone();
    let http_port = config.http_port;
    tokio::spawn(async move {
        metrics::run_http_metrics_server(http_port, http_state, http_aggregator).await;
    });

    // Start gRPC server for TUI queries
    let addr: SocketAddr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    let service = CollectorServiceImpl {
        state: state.clone(),
        aggregator: aggregator.clone(),
        start_time,
    };

    info!(%addr, "gRPC server listening");

    tonic::transport::Server::builder()
        .add_service(CollectorServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
