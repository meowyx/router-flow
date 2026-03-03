use std::sync::Arc;

use tokio::sync::Mutex;

use crate::aggregator::Aggregator;
use crate::state::CollectorState;

/// Render Prometheus text exposition format from the aggregator.
pub fn render_prometheus(aggregator: &mut Aggregator, total_couriers: usize) -> String {
    let mut out = String::with_capacity(1024);

    // assignments_total
    out.push_str("# HELP assignments_total Total number of assignments made.\n");
    out.push_str("# TYPE assignments_total counter\n");
    out.push_str(&format!(
        "assignments_total {}\n",
        aggregator.total_assignments()
    ));

    // events_processed_total
    out.push_str("# HELP events_processed_total Total events received.\n");
    out.push_str("# TYPE events_processed_total counter\n");
    out.push_str(&format!(
        "events_processed_total {}\n",
        aggregator.total_events_processed()
    ));

    // assignment_latency_seconds (avg, p95, p99 as gauges for simplicity)
    let avg = aggregator.avg_latency_ms();
    let p95 = aggregator.latency_percentile(0.95);
    let p99 = aggregator.latency_percentile(0.99);

    out.push_str(
        "# HELP assignment_latency_ms Assignment latency from order creation to assignment.\n",
    );
    out.push_str("# TYPE assignment_latency_ms gauge\n");
    out.push_str(&format!(
        "assignment_latency_ms{{quantile=\"avg\"}} {:.2}\n",
        avg
    ));
    out.push_str(&format!(
        "assignment_latency_ms{{quantile=\"0.95\"}} {:.2}\n",
        p95
    ));
    out.push_str(&format!(
        "assignment_latency_ms{{quantile=\"0.99\"}} {:.2}\n",
        p99
    ));

    // courier_utilization
    let util = aggregator.courier_utilization_pct(total_couriers);
    out.push_str("# HELP courier_utilization Percentage of couriers with assignments in window.\n");
    out.push_str("# TYPE courier_utilization gauge\n");
    out.push_str(&format!("courier_utilization {:.2}\n", util));

    // avg_score
    let avg_score = aggregator.avg_score();
    out.push_str("# HELP avg_assignment_score Average assignment score in window.\n");
    out.push_str("# TYPE avg_assignment_score gauge\n");
    out.push_str(&format!("avg_assignment_score {:.4}\n", avg_score));

    out
}

/// Start a minimal HTTP server that serves `/metrics` in Prometheus format.
pub async fn run_http_metrics_server(
    port: u16,
    state: Arc<CollectorState>,
    aggregator: Arc<Mutex<Aggregator>>,
) {
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Request, Response, Server, StatusCode};

    let make_svc = make_service_fn(move |_conn| {
        let state = state.clone();
        let aggregator = aggregator.clone();

        async move {
            Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| {
                let state = state.clone();
                let aggregator = aggregator.clone();

                async move {
                    match req.uri().path() {
                        "/metrics" => {
                            let mut agg = aggregator.lock().await;
                            let total_couriers = state.total_couriers.load(std::sync::atomic::Ordering::Relaxed);
                            let body = render_prometheus(&mut agg, total_couriers);

                            Ok::<_, hyper::Error>(
                                Response::builder()
                                    .status(StatusCode::OK)
                                    .header("content-type", "text/plain; charset=utf-8")
                                    .body(Body::from(body))
                                    .unwrap(),
                            )
                        }
                        "/health" => Ok(Response::builder()
                            .status(StatusCode::OK)
                            .body(Body::from("ok"))
                            .unwrap()),
                        _ => Ok(Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .body(Body::from("not found"))
                            .unwrap()),
                    }
                }
            }))
        }
    });

    let addr = ([0, 0, 0, 0], port).into();
    tracing::info!(%port, "HTTP metrics server listening");

    if let Err(e) = Server::bind(&addr).serve(make_svc).await {
        tracing::error!(error = %e, "HTTP metrics server error");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregator::Aggregator;

    #[test]
    fn prometheus_output_contains_expected_metrics() {
        let mut agg = Aggregator::new(60);
        agg.record_assignment("c1".to_string(), 0.85, 120);
        agg.record_assignment("c2".to_string(), 0.70, 80);

        let output = render_prometheus(&mut agg, 10);

        assert!(output.contains("assignments_total 2"));
        assert!(output.contains("events_processed_total 2"));
        assert!(output.contains("assignment_latency_ms{quantile=\"avg\"}"));
        assert!(output.contains("courier_utilization"));
        assert!(output.contains("avg_assignment_score"));
    }

    #[test]
    fn prometheus_output_empty_aggregator() {
        let mut agg = Aggregator::new(60);
        let output = render_prometheus(&mut agg, 0);

        assert!(output.contains("assignments_total 0"));
        assert!(output.contains("events_processed_total 0"));
        assert!(output.contains("courier_utilization 0.00"));
    }
}
