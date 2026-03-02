use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub num_couriers: usize,
    pub city_center_lat: f64,
    pub city_center_lng: f64,
    pub city_radius_km: f64,
    pub tick_interval_ms: u64,
    pub movement_speed: f64,
    pub grpc_port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        Self {
            num_couriers: parse_or("NUM_COURIERS", 10),
            city_center_lat: parse_or("CITY_CENTER_LAT", 52.52),
            city_center_lng: parse_or("CITY_CENTER_LNG", 13.405),
            city_radius_km: parse_or("CITY_RADIUS_KM", 5.0),
            tick_interval_ms: parse_or("TICK_INTERVAL_MS", 500),
            movement_speed: parse_or("MOVEMENT_SPEED", 0.001),
            grpc_port: parse_or("GRPC_PORT", 50052),
        }
    }

    /// Approximate radius in degrees (1 degree latitude ~ 111 km)
    pub fn city_radius_deg(&self) -> f64 {
        self.city_radius_km / 111.0
    }
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
