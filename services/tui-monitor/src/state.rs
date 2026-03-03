use std::collections::VecDeque;

use chrono::{DateTime, Local};

/// Maximum number of log entries to keep in the TUI.
const MAX_LOG_ENTRIES: usize = 500;

/// A single entry in the unified event log.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub source: LogSource,
    pub message: String,
}

/// Which service produced this log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    CitySim,
    OrderGen,
    Optimizer,
    Collector,
    System,
}

impl LogSource {
    pub fn label(&self) -> &'static str {
        match self {
            LogSource::CitySim => "city-sim",
            LogSource::OrderGen => "order-gen",
            LogSource::Optimizer => "optimizer",
            LogSource::Collector => "collector",
            LogSource::System => "system",
        }
    }
}

/// Stats from the City Simulator panel.
#[derive(Debug, Clone, Default)]
pub struct CitySimStats {
    pub total_couriers: u32,
    pub idle_count: u32,
    pub en_route_count: u32,
    pub connected: bool,
}

/// Stats from the Order Generator panel.
#[derive(Debug, Clone, Default)]
pub struct OrderGenStats {
    pub total_orders_seen: u64,
    pub last_order_summary: String,
    pub connected: bool,
}

/// Stats from the Assignment Optimizer panel.
#[derive(Debug, Clone, Default)]
pub struct OptimizerStats {
    pub total_assignments: u64,
    pub last_assignment_summary: String,
    pub last_score: f64,
    pub connected: bool,
}

/// Stats from the Event Collector panel.
#[derive(Debug, Clone, Default)]
pub struct CollectorStats {
    pub total_assignments: i64,
    pub total_events_processed: i64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub courier_utilization_pct: f64,
    pub avg_score: f64,
    pub uptime_seconds: i64,
    pub connected: bool,
}

/// Focused panel for keyboard navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPanel {
    CitySim,
    OrderGen,
    Optimizer,
    Collector,
    Log,
}

impl FocusPanel {
    pub fn next(self) -> Self {
        match self {
            FocusPanel::CitySim => FocusPanel::Optimizer,
            FocusPanel::Optimizer => FocusPanel::OrderGen,
            FocusPanel::OrderGen => FocusPanel::Collector,
            FocusPanel::Collector => FocusPanel::Log,
            FocusPanel::Log => FocusPanel::CitySim,
        }
    }
}

/// All TUI state, mutated from stream consumers and read by the renderer.
pub struct TuiState {
    pub city_sim: CitySimStats,
    pub order_gen: OrderGenStats,
    pub optimizer: OptimizerStats,
    pub collector: CollectorStats,
    pub log_entries: VecDeque<LogEntry>,
    pub focus: FocusPanel,
    pub auto_scroll: bool,
    pub log_scroll_offset: u16,
}

impl TuiState {
    pub fn new() -> Self {
        Self {
            city_sim: CitySimStats::default(),
            order_gen: OrderGenStats::default(),
            optimizer: OptimizerStats::default(),
            collector: CollectorStats::default(),
            log_entries: VecDeque::new(),
            focus: FocusPanel::Log,
            auto_scroll: true,
            log_scroll_offset: 0,
        }
    }

    /// Push a log entry, evicting old entries if over capacity.
    pub fn push_log(&mut self, source: LogSource, message: String) {
        self.log_entries.push_back(LogEntry {
            timestamp: Local::now(),
            source,
            message,
        });
        while self.log_entries.len() > MAX_LOG_ENTRIES {
            self.log_entries.pop_front();
        }
    }
}
