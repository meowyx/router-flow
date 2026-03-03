use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    /// gRPC server port (for TUI Monitor to query metrics)
    pub grpc_port: u16,
    /// HTTP port for Prometheus /metrics endpoint
    pub http_port: u16,
    /// Assignment Optimizer gRPC address (WatchAssignments stream)
    pub optimizer_addr: String,
    /// Sliding window size in seconds for latency percentile calculations
    pub window_size_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        Self {
            grpc_port: parse_or("GRPC_PORT", 50054),
            http_port: parse_or("HTTP_PORT", 3001),
            optimizer_addr: env::var("OPTIMIZER_ADDR")
                .unwrap_or_else(|_| "http://localhost:50051".to_string()),
            window_size_secs: parse_or("WINDOW_SIZE_SECS", 60),
        }
    }
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
