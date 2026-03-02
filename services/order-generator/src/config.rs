use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub order_rate_per_sec: f64,
    pub pattern: Pattern,
    pub city_center_lat: f64,
    pub city_center_lng: f64,
    pub city_radius_km: f64,
    pub grpc_port: u16,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Uniform,
    Hotspot,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();

        let pattern = match env::var("PATTERN")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "hotspot" => Pattern::Hotspot,
            _ => Pattern::Uniform,
        };

        Self {
            order_rate_per_sec: parse_or("ORDER_RATE_PER_SEC", 1.0),
            pattern,
            city_center_lat: parse_or("CITY_CENTER_LAT", 52.52),
            city_center_lng: parse_or("CITY_CENTER_LNG", 13.405),
            city_radius_km: parse_or("CITY_RADIUS_KM", 5.0),
            grpc_port: parse_or("GRPC_PORT", 50053),
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
