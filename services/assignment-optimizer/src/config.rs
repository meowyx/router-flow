use std::env;

use router_flow_shared::engine::scoring::ScoringWeights;

#[derive(Debug, Clone)]
pub struct Config {
    pub grpc_port: u16,
    pub city_simulator_addr: String,
    pub order_generator_addr: String,
    pub scoring_weights: ScoringWeights,
    pub assignment_interval_ms: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        Self {
            grpc_port: parse_or("GRPC_PORT", 50051),
            city_simulator_addr: env::var("CITY_SIMULATOR_ADDR")
                .unwrap_or_else(|_| "http://localhost:50052".to_string()),
            order_generator_addr: env::var("ORDER_GENERATOR_ADDR")
                .unwrap_or_else(|_| "http://localhost:50053".to_string()),
            scoring_weights: ScoringWeights {
                distance: parse_or("WEIGHT_DISTANCE", 0.40),
                load: parse_or("WEIGHT_LOAD", 0.30),
                rating: parse_or("WEIGHT_RATING", 0.20),
                priority: parse_or("WEIGHT_PRIORITY", 0.10),
            },
            assignment_interval_ms: parse_or("ASSIGNMENT_INTERVAL_MS", 1000),
        }
    }
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
