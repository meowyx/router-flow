use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub city_simulator_addr: String,
    pub order_generator_addr: String,
    pub optimizer_addr: String,
    pub collector_addr: String,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        Self {
            city_simulator_addr: env::var("CITY_SIMULATOR_ADDR")
                .unwrap_or_else(|_| "http://localhost:50052".to_string()),
            order_generator_addr: env::var("ORDER_GENERATOR_ADDR")
                .unwrap_or_else(|_| "http://localhost:50053".to_string()),
            optimizer_addr: env::var("OPTIMIZER_ADDR")
                .unwrap_or_else(|_| "http://localhost:50051".to_string()),
            collector_addr: env::var("COLLECTOR_ADDR")
                .unwrap_or_else(|_| "http://localhost:50054".to_string()),
        }
    }
}
