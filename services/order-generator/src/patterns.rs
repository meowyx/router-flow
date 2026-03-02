use chrono::Utc;
use rand::Rng;
use uuid::Uuid;

use router_flow_proto::order::{GeoPoint as ProtoGeoPoint, NewOrder};

use crate::config::{Config, Pattern};

/// Berlin hotspot locations for the hotspot generation pattern
const HOTSPOTS: &[(f64, f64)] = &[
    (52.520, 13.405), // Mitte (business district)
    (52.522, 13.413), // Alexanderplatz
    (52.509, 13.376), // Potsdamer Platz
    (52.525, 13.369), // Hauptbahnhof (main station)
    (52.507, 13.332), // Charlottenburg
];

/// Priority levels and their string representations
const PRIORITIES: &[&str] = &["low", "normal", "high", "urgent"];

/// Generate a single order based on the configured pattern
pub fn generate_order(config: &Config) -> NewOrder {
    let mut rng = rand::thread_rng();

    let (pickup, dropoff) = match config.pattern {
        Pattern::Uniform => generate_uniform(config, &mut rng),
        Pattern::Hotspot => generate_hotspot(config, &mut rng),
    };

    let priority = pick_priority(&config.pattern, &mut rng);

    NewOrder {
        order_id: Uuid::new_v4().to_string(),
        pickup: Some(pickup),
        dropoff: Some(dropoff),
        priority: priority.to_string(),
        created_at_ms: Utc::now().timestamp_millis(),
    }
}

/// Uniform pattern: both pickup and dropoff are random points within city bounds
fn generate_uniform(config: &Config, rng: &mut impl Rng) -> (ProtoGeoPoint, ProtoGeoPoint) {
    let radius = config.city_radius_deg();
    let pickup = random_point(config.city_center_lat, config.city_center_lng, radius, rng);
    let dropoff = random_point(config.city_center_lat, config.city_center_lng, radius, rng);
    (pickup, dropoff)
}

/// Hotspot pattern: pickup clusters near a random hotspot, dropoff is random within city
fn generate_hotspot(config: &Config, rng: &mut impl Rng) -> (ProtoGeoPoint, ProtoGeoPoint) {
    let radius = config.city_radius_deg();

    // Pick a random hotspot for pickup
    let hotspot = HOTSPOTS[rng.gen_range(0..HOTSPOTS.len())];
    let hotspot_spread = 0.005; // ~500m spread around the hotspot
    let pickup = random_point(hotspot.0, hotspot.1, hotspot_spread, rng);

    // Dropoff goes anywhere in the city
    let dropoff = random_point(config.city_center_lat, config.city_center_lng, radius, rng);

    (pickup, dropoff)
}

/// Pick a priority level based on the pattern
fn pick_priority(pattern: &Pattern, rng: &mut impl Rng) -> &'static str {
    let roll: f64 = rng.r#gen();
    match pattern {
        // Uniform: 70% normal, 20% high, 7% urgent, 3% low
        Pattern::Uniform => {
            if roll < 0.03 {
                PRIORITIES[0] // low
            } else if roll < 0.73 {
                PRIORITIES[1] // normal
            } else if roll < 0.93 {
                PRIORITIES[2] // high
            } else {
                PRIORITIES[3] // urgent
            }
        }
        // Hotspot: busier areas mean more urgency — 50% normal, 30% high, 15% urgent, 5% low
        Pattern::Hotspot => {
            if roll < 0.05 {
                PRIORITIES[0] // low
            } else if roll < 0.55 {
                PRIORITIES[1] // normal
            } else if roll < 0.85 {
                PRIORITIES[2] // high
            } else {
                PRIORITIES[3] // urgent
            }
        }
    }
}

/// Generate a random point within a circle (center +/- radius in degrees)
fn random_point(
    center_lat: f64,
    center_lng: f64,
    radius_deg: f64,
    rng: &mut impl Rng,
) -> ProtoGeoPoint {
    let angle: f64 = rng.gen_range(0.0..std::f64::consts::TAU);
    let r: f64 = radius_deg * rng.gen_range(0.0..1.0_f64).sqrt();

    ProtoGeoPoint {
        latitude: center_lat + r * angle.sin(),
        longitude: center_lng + r * angle.cos(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform_config() -> Config {
        Config {
            order_rate_per_sec: 1.0,
            pattern: Pattern::Uniform,
            city_center_lat: 52.52,
            city_center_lng: 13.405,
            city_radius_km: 5.0,
            grpc_port: 50053,
        }
    }

    fn hotspot_config() -> Config {
        Config {
            order_rate_per_sec: 1.0,
            pattern: Pattern::Hotspot,
            city_center_lat: 52.52,
            city_center_lng: 13.405,
            city_radius_km: 5.0,
            grpc_port: 50053,
        }
    }

    #[test]
    fn uniform_orders_within_city_bounds() {
        let config = uniform_config();
        let radius = config.city_radius_deg();

        for _ in 0..500 {
            let order = generate_order(&config);
            let pickup = order.pickup.unwrap();
            let dropoff = order.dropoff.unwrap();

            let dp_lat = (pickup.latitude - config.city_center_lat).abs();
            let dp_lng = (pickup.longitude - config.city_center_lng).abs();
            assert!(dp_lat <= radius && dp_lng <= radius, "pickup out of bounds");

            let dd_lat = (dropoff.latitude - config.city_center_lat).abs();
            let dd_lng = (dropoff.longitude - config.city_center_lng).abs();
            assert!(
                dd_lat <= radius && dd_lng <= radius,
                "dropoff out of bounds"
            );
        }
    }

    #[test]
    fn hotspot_pickup_near_hotspot_center() {
        let config = hotspot_config();

        for _ in 0..500 {
            let order = generate_order(&config);
            let pickup = order.pickup.unwrap();

            // Pickup should be within ~1km of some hotspot
            let near_any_hotspot = HOTSPOTS.iter().any(|(lat, lng)| {
                let dlat = (pickup.latitude - lat).abs();
                let dlng = (pickup.longitude - lng).abs();
                dlat < 0.01 && dlng < 0.01 // ~1km
            });

            assert!(
                near_any_hotspot,
                "pickup not near any hotspot: ({}, {})",
                pickup.latitude, pickup.longitude
            );
        }
    }

    #[test]
    fn orders_have_valid_priority() {
        let config = uniform_config();
        let valid = ["low", "normal", "high", "urgent"];

        for _ in 0..100 {
            let order = generate_order(&config);
            assert!(
                valid.contains(&order.priority.as_str()),
                "invalid priority: {}",
                order.priority
            );
        }
    }

    #[test]
    fn orders_have_unique_ids() {
        let config = uniform_config();
        let ids: Vec<String> = (0..100).map(|_| generate_order(&config).order_id).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "order IDs should be unique");
    }

    #[test]
    fn priority_distribution_is_reasonable() {
        let config = uniform_config();
        let mut counts = std::collections::HashMap::new();

        for _ in 0..1000 {
            let order = generate_order(&config);
            *counts.entry(order.priority).or_insert(0) += 1;
        }

        // Uniform: ~70% normal, so at least 500 out of 1000
        let normal = counts.get("normal").copied().unwrap_or(0);
        assert!(
            normal > 500,
            "expected majority normal priority, got {normal}/1000"
        );

        // Urgent should be rare: < 15%
        let urgent = counts.get("urgent").copied().unwrap_or(0);
        assert!(urgent < 150, "expected few urgent, got {urgent}/1000");
    }
}
