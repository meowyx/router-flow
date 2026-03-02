mod config;
mod generator;
mod grpc;
mod patterns;

use std::net::SocketAddr;

use tokio::sync::broadcast;
use tracing::info;

use router_flow_proto::order::order_service_server::OrderServiceServer;
use router_flow_proto::order::NewOrder;

use config::Config;
use grpc::OrderServiceImpl;

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

    info!(?config, "order generator starting");

    let (order_tx, _) = broadcast::channel::<NewOrder>(256);

    // Spawn the order generation loop
    let generator_config = config.clone();
    let generator_tx = order_tx.clone();
    tokio::spawn(async move {
        generator::run_generator(generator_config, generator_tx).await;
    });

    // Start gRPC server
    let addr: SocketAddr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    let service = OrderServiceImpl { order_tx };

    info!(%addr, "gRPC server listening");

    tonic::transport::Server::builder()
        .add_service(OrderServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
