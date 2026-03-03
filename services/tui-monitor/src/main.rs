mod config;
mod state;
mod streams;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use config::Config;
use state::{LogSource, TuiState};
use streams::TuiEvent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, config).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut state = TuiState::new();
    state.push_log(LogSource::System, "TUI monitor starting...".to_string());

    // Channel for all gRPC stream events
    let (tx, mut rx) = mpsc::channel::<TuiEvent>(512);

    // Spawn all gRPC stream consumers
    let tx1 = tx.clone();
    let addr1 = config.city_simulator_addr.clone();
    tokio::spawn(async move { streams::run_location_stream(tx1, addr1).await });

    let tx2 = tx.clone();
    let addr2 = config.order_generator_addr.clone();
    tokio::spawn(async move { streams::run_order_stream(tx2, addr2).await });

    let tx3 = tx.clone();
    let addr3 = config.optimizer_addr.clone();
    tokio::spawn(async move { streams::run_assignment_stream(tx3, addr3).await });

    let tx4 = tx.clone();
    let addr4 = config.collector_addr.clone();
    tokio::spawn(async move { streams::run_collector_event_stream(tx4, addr4).await });

    let tx5 = tx.clone();
    let addr5 = config.collector_addr.clone();
    tokio::spawn(async move { streams::run_metrics_poller(tx5, addr5).await });

    // Drop the original tx so channel closes when all spawned tasks end
    drop(tx);

    state.push_log(
        LogSource::System,
        "connecting to services...".to_string(),
    );

    loop {
        // Render
        terminal.draw(|frame| ui::draw(frame, &state))?;

        // Process pending gRPC events (non-blocking drain)
        loop {
            match rx.try_recv() {
                Ok(event) => apply_event(&mut state, event),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    state.push_log(LogSource::System, "all streams disconnected".to_string());
                    return Ok(());
                }
            }
        }

        // Process keyboard input (with timeout for responsive rendering)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Tab => {
                            state.focus = state.focus.next();
                        }
                        KeyCode::Char('p') => {
                            state.auto_scroll = !state.auto_scroll;
                        }
                        KeyCode::Up => {
                            if !state.auto_scroll {
                                state.log_scroll_offset =
                                    state.log_scroll_offset.saturating_add(1);
                            }
                        }
                        KeyCode::Down => {
                            if !state.auto_scroll {
                                state.log_scroll_offset =
                                    state.log_scroll_offset.saturating_sub(1);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Apply a TuiEvent to the TUI state.
fn apply_event(state: &mut TuiState, event: TuiEvent) {
    match event {
        TuiEvent::LocationBatch {
            total,
            idle,
            en_route,
        } => {
            state.city_sim.total_couriers = total;
            state.city_sim.idle_count = idle;
            state.city_sim.en_route_count = en_route;
            // Only log occasionally (every batch would flood the log)
        }

        TuiEvent::CitySimConnected(connected) => {
            state.city_sim.connected = connected;
            let msg = if connected {
                "connected to city simulator"
            } else {
                "disconnected from city simulator"
            };
            state.push_log(LogSource::CitySim, msg.to_string());
        }

        TuiEvent::NewOrder { order_id, priority } => {
            state.order_gen.total_orders_seen += 1;
            let summary = format!(
                "order={} priority={}",
                short_id(&order_id),
                priority
            );
            state.order_gen.last_order_summary = summary.clone();
            state.push_log(LogSource::OrderGen, format!("created {}", summary));
        }

        TuiEvent::OrderGenConnected(connected) => {
            state.order_gen.connected = connected;
            let msg = if connected {
                "connected to order generator"
            } else {
                "disconnected from order generator"
            };
            state.push_log(LogSource::OrderGen, msg.to_string());
        }

        TuiEvent::Assignment {
            order_id,
            courier_id,
            score,
        } => {
            state.optimizer.total_assignments += 1;
            state.optimizer.last_score = score;
            let summary = format!(
                "order={} -> courier={} score={:.2}",
                short_id(&order_id),
                short_id(&courier_id),
                score
            );
            state.optimizer.last_assignment_summary = summary.clone();
            state.push_log(LogSource::Optimizer, format!("assigned {}", summary));
        }

        TuiEvent::OptimizerConnected(connected) => {
            state.optimizer.connected = connected;
            let msg = if connected {
                "connected to assignment optimizer"
            } else {
                "disconnected from assignment optimizer"
            };
            state.push_log(LogSource::Optimizer, msg.to_string());
        }

        TuiEvent::CollectorEvent { summary } => {
            state.push_log(LogSource::Collector, summary);
        }

        TuiEvent::CollectorMetrics {
            total_assignments,
            total_events_processed,
            avg_latency_ms,
            p95_latency_ms,
            courier_utilization_pct,
            avg_score,
            uptime_seconds,
        } => {
            state.collector.total_assignments = total_assignments;
            state.collector.total_events_processed = total_events_processed;
            state.collector.avg_latency_ms = avg_latency_ms;
            state.collector.p95_latency_ms = p95_latency_ms;
            state.collector.courier_utilization_pct = courier_utilization_pct;
            state.collector.avg_score = avg_score;
            state.collector.uptime_seconds = uptime_seconds;
        }

        TuiEvent::CollectorConnected(connected) => {
            state.collector.connected = connected;
            let msg = if connected {
                "connected to event collector"
            } else {
                "disconnected from event collector"
            };
            state.push_log(LogSource::Collector, msg.to_string());
        }
    }
}

/// Shorten a UUID string for display readability.
fn short_id(id: &str) -> &str {
    if id.len() > 8 {
        &id[..8]
    } else {
        id
    }
}
