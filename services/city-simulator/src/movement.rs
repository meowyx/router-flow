use rand::Rng;
use router_flow_shared::models::courier::GeoPoint;

use crate::config::Config;
use crate::courier::{MovementState, SimCourier};

impl SimCourier {
    /// Create a new courier at a random position within the city
    pub fn new_random(config: &Config) -> Self {
        let mut rng = rand::thread_rng();
        let radius = config.city_radius_deg();

        let position = random_point_in_city(
            config.city_center_lat,
            config.city_center_lng,
            radius,
            &mut rng,
        );
        let waypoint = random_point_in_city(
            config.city_center_lat,
            config.city_center_lng,
            radius,
            &mut rng,
        );

        Self {
            id: uuid::Uuid::new_v4(),
            position,
            waypoint,
            state: MovementState::Idle,
            capacity: rng.gen_range(2..=5),
            current_load: 0,
            rating: rng.gen_range(3.0..=5.0),
        }
    }

    /// Move one step toward the current waypoint. Returns true if waypoint was reached.
    pub fn step(&mut self, speed: f64) -> bool {
        let dx = self.waypoint.lng - self.position.lng;
        let dy = self.waypoint.lat - self.position.lat;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < speed * 0.5 {
            self.position = self.waypoint.clone();
            return true;
        }

        let nx = dx / dist;
        let ny = dy / dist;
        self.position.lng += nx * speed;
        self.position.lat += ny * speed;

        false
    }

    /// Handle waypoint arrival based on current movement state
    pub fn on_waypoint_reached(&mut self, config: &Config) {
        let mut rng = rand::thread_rng();
        let radius = config.city_radius_deg();

        match &self.state {
            MovementState::Idle => {
                self.waypoint = random_point_in_city(
                    config.city_center_lat,
                    config.city_center_lng,
                    radius,
                    &mut rng,
                );
            }
            MovementState::EnRoutePickup {
                dropoff, order_id, ..
            } => {
                let dropoff = dropoff.clone();
                let order_id = order_id.clone();
                self.current_load = self.current_load.saturating_add(1);
                self.waypoint = dropoff.clone();
                self.state = MovementState::EnRouteDropoff { order_id, dropoff };
            }
            MovementState::EnRouteDropoff { .. } => {
                self.current_load = self.current_load.saturating_sub(1);
                self.state = MovementState::Idle;
                self.waypoint = random_point_in_city(
                    config.city_center_lat,
                    config.city_center_lng,
                    radius,
                    &mut rng,
                );
            }
        }
    }
}

/// Generate a random point within city bounds
pub fn random_point_in_city(
    center_lat: f64,
    center_lng: f64,
    radius_deg: f64,
    rng: &mut impl Rng,
) -> GeoPoint {
    let angle: f64 = rng.gen_range(0.0..std::f64::consts::TAU);
    let r: f64 = rng.gen_range(0.0..radius_deg) * rng.gen_range(0.5..1.0_f64).sqrt();

    GeoPoint {
        lat: center_lat + r * angle.sin(),
        lng: center_lng + r * angle.cos(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            num_couriers: 5,
            city_center_lat: 52.52,
            city_center_lng: 13.405,
            city_radius_km: 5.0,
            tick_interval_ms: 500,
            movement_speed: 0.001,
            grpc_port: 50052,
        }
    }

    #[test]
    fn courier_moves_toward_waypoint() {
        let config = test_config();
        let mut courier = SimCourier::new_random(&config);
        courier.position = GeoPoint {
            lat: 52.52,
            lng: 13.40,
        };
        courier.waypoint = GeoPoint {
            lat: 52.53,
            lng: 13.40,
        };

        let initial_dist = (courier.waypoint.lat - courier.position.lat).abs();
        courier.step(0.001);
        let new_dist = (courier.waypoint.lat - courier.position.lat).abs();

        assert!(
            new_dist < initial_dist,
            "courier should be closer after step"
        );
    }

    #[test]
    fn courier_reaches_waypoint() {
        let config = test_config();
        let mut courier = SimCourier::new_random(&config);
        courier.position = GeoPoint {
            lat: 52.52,
            lng: 13.40,
        };
        courier.waypoint = GeoPoint {
            lat: 52.52001,
            lng: 13.40,
        };

        let reached = courier.step(0.001);
        assert!(reached, "courier should reach a very close waypoint");
    }

    #[test]
    fn idle_courier_picks_new_waypoint_on_arrival() {
        let config = test_config();
        let mut courier = SimCourier::new_random(&config);
        courier.state = MovementState::Idle;
        let old_waypoint = courier.waypoint.clone();

        courier.on_waypoint_reached(&config);

        let changed = (courier.waypoint.lat - old_waypoint.lat).abs() > 1e-10
            || (courier.waypoint.lng - old_waypoint.lng).abs() > 1e-10;
        assert!(changed, "idle courier should get a new waypoint");
    }

    #[test]
    fn assign_courier_transitions_to_dropoff_on_pickup_arrival() {
        let config = test_config();
        let mut courier = SimCourier::new_random(&config);
        let dropoff = GeoPoint {
            lat: 52.53,
            lng: 13.41,
        };

        courier.state = MovementState::EnRoutePickup {
            order_id: "order-1".to_string(),
            pickup: courier.position.clone(),
            dropoff: dropoff.clone(),
        };

        courier.on_waypoint_reached(&config);

        assert!(matches!(
            courier.state,
            MovementState::EnRouteDropoff { .. }
        ));
        assert_eq!(courier.current_load, 1);
        assert!((courier.waypoint.lat - dropoff.lat).abs() < 1e-10);
    }

    #[test]
    fn courier_returns_to_idle_after_dropoff() {
        let config = test_config();
        let mut courier = SimCourier::new_random(&config);
        courier.current_load = 1;
        courier.state = MovementState::EnRouteDropoff {
            order_id: "order-1".to_string(),
            dropoff: courier.position.clone(),
        };

        courier.on_waypoint_reached(&config);

        assert!(matches!(courier.state, MovementState::Idle));
        assert_eq!(courier.current_load, 0);
    }

    #[test]
    fn random_point_stays_within_bounds() {
        let mut rng = rand::thread_rng();
        let center_lat = 52.52;
        let center_lng = 13.405;
        let radius_deg = 5.0 / 111.0;

        for _ in 0..1000 {
            let p = random_point_in_city(center_lat, center_lng, radius_deg, &mut rng);
            let dlat = (p.lat - center_lat).abs();
            let dlng = (p.lng - center_lng).abs();
            assert!(
                dlat <= radius_deg && dlng <= radius_deg,
                "point ({}, {}) exceeds city bounds",
                p.lat,
                p.lng
            );
        }
    }
}
