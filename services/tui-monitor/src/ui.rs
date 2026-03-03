use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::state::{FocusPanel, LogSource, TuiState};

/// Render the full TUI layout.
pub fn draw(frame: &mut Frame, state: &TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // top panels row
            Constraint::Length(8), // bottom panels row
            Constraint::Min(6),    // log panel
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    // Top row: City Sim (left) | Optimizer (right)
    let top_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    draw_city_sim_panel(frame, top_row[0], state);
    draw_optimizer_panel(frame, top_row[1], state);

    // Bottom row: Order Gen (left) | Collector (right)
    let bottom_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    draw_order_gen_panel(frame, bottom_row[0], state);
    draw_collector_panel(frame, bottom_row[1], state);

    // Log panel
    draw_log_panel(frame, chunks[2], state);

    // Status bar
    draw_status_bar(frame, chunks[3], state);
}

fn panel_border_style(state: &TuiState, panel: FocusPanel) -> Style {
    if state.focus == panel {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn connection_indicator(connected: bool) -> Span<'static> {
    if connected {
        Span::styled(" [connected]", Style::default().fg(Color::Green))
    } else {
        Span::styled(" [disconnected]", Style::default().fg(Color::Red))
    }
}

fn draw_city_sim_panel(frame: &mut Frame, area: Rect, state: &TuiState) {
    let s = &state.city_sim;

    let title = Line::from(vec![
        Span::styled(
            "City Simulator",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        connection_indicator(s.connected),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(panel_border_style(state, FocusPanel::CitySim));

    let text = vec![
        Line::from(format!("Couriers: {} active", s.total_couriers)),
        Line::from(format!(
            "Idle: {}  En-route: {}",
            s.idle_count, s.en_route_count
        )),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_optimizer_panel(frame: &mut Frame, area: Rect, state: &TuiState) {
    let s = &state.optimizer;

    let title = Line::from(vec![
        Span::styled(
            "Assignment Optimizer",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        connection_indicator(s.connected),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(panel_border_style(state, FocusPanel::Optimizer));

    let text = vec![
        Line::from(format!("Assignments: {}", s.total_assignments)),
        Line::from(format!("Last score: {:.2}", s.last_score)),
        Line::from(""),
        Line::from(truncate(
            &s.last_assignment_summary,
            area.width as usize - 4,
        )),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_order_gen_panel(frame: &mut Frame, area: Rect, state: &TuiState) {
    let s = &state.order_gen;

    let title = Line::from(vec![
        Span::styled(
            "Order Generator",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        connection_indicator(s.connected),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(panel_border_style(state, FocusPanel::OrderGen));

    let text = vec![
        Line::from(format!("Orders seen: {}", s.total_orders_seen)),
        Line::from(""),
        Line::from(truncate(&s.last_order_summary, area.width as usize - 4)),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_collector_panel(frame: &mut Frame, area: Rect, state: &TuiState) {
    let s = &state.collector;

    let title = Line::from(vec![
        Span::styled(
            "Event Collector",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        connection_indicator(s.connected),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(panel_border_style(state, FocusPanel::Collector));

    let text = vec![
        Line::from(format!(
            "Assignments: {}  Events: {}",
            s.total_assignments, s.total_events_processed
        )),
        Line::from(format!(
            "Avg latency: {:.1}ms  p95: {:.1}ms",
            s.avg_latency_ms, s.p95_latency_ms
        )),
        Line::from(format!(
            "Utilization: {:.1}%  Avg score: {:.2}",
            s.courier_utilization_pct, s.avg_score
        )),
        Line::from(format!("Uptime: {}s", s.uptime_seconds)),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_log_panel(frame: &mut Frame, area: Rect, state: &TuiState) {
    let title = Line::from(vec![
        Span::styled(
            "Live Event Log",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        if state.auto_scroll {
            Span::styled(" [auto-scroll]", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(" [paused]", Style::default().fg(Color::Yellow))
        },
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(panel_border_style(state, FocusPanel::Log));

    // Available height inside the block borders
    let inner_height = area.height.saturating_sub(2) as usize;

    let items: Vec<ListItem> = state
        .log_entries
        .iter()
        .rev()
        .skip(state.log_scroll_offset as usize)
        .take(inner_height)
        .map(|entry| {
            let time = entry.timestamp.format("%H:%M:%S");
            let label = entry.source.label();
            let color = match entry.source {
                LogSource::CitySim => Color::Blue,
                LogSource::OrderGen => Color::Green,
                LogSource::Optimizer => Color::Yellow,
                LogSource::Collector => Color::Magenta,
                LogSource::System => Color::DarkGray,
            };

            let line = Line::from(vec![
                Span::styled(format!("{} ", time), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("[{}] ", label),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&entry.message),
            ]);

            ListItem::new(line)
        })
        .collect();

    // Reverse so newest is at the bottom
    let items: Vec<ListItem> = items.into_iter().rev().collect();
    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_status_bar(frame: &mut Frame, area: Rect, state: &TuiState) {
    let focus_label = match state.focus {
        FocusPanel::CitySim => "City Sim",
        FocusPanel::OrderGen => "Order Gen",
        FocusPanel::Optimizer => "Optimizer",
        FocusPanel::Collector => "Collector",
        FocusPanel::Log => "Log",
    };

    let bar = Line::from(vec![
        Span::styled(
            " q",
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" quit  "),
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" focus  "),
        Span::styled(
            "^/v",
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" scroll  "),
        Span::styled(
            "p",
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" pause  "),
        Span::styled(
            format!("  Focus: {}", focus_label),
            Style::default().fg(Color::Cyan),
        ),
    ]);

    let paragraph = Paragraph::new(bar);
    frame.render_widget(paragraph, area);
}

/// Truncate a string to fit within a given width, adding "..." if truncated.
fn truncate(s: &str, max_width: usize) -> String {
    if max_width < 4 {
        return String::new();
    }
    if s.len() <= max_width {
        s.to_string()
    } else {
        format!("{}...", &s[..max_width - 3])
    }
}
