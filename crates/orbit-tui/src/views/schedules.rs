use crate::app::App;
use orbit_core::schedule::{ScheduledPlan, format_schedule, format_ts};
use ratatui::{
    Frame,
    layout::Constraint,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table},
};

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn next_run_label(sched: &ScheduledPlan) -> String {
    match sched.next_run {
        Some(ts) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if ts <= now {
                "now".to_string()
            } else {
                format!("in {}s", ts - now)
            }
        }
        None => "—".to_string(),
    }
}

pub fn render(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let label = app.palette.label;

    let count = app.schedules.schedules.len();
    let title = if count == 0 {
        " Schedules — none ".to_string()
    } else {
        format!(" Schedules — {count} ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim))
        .title(Span::styled(title, Style::default().fg(dim)))
        .padding(Padding::uniform(1));

    if app.schedules.schedules.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No schedules. Create one with: orbit plan schedule create \"<intent>\" --cron \"0 9 * * 1-5\"",
                Style::default().fg(dim),
            )),
        ])
        .block(block);
        f.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("SCHEDULE").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Cell::from("NEXT RUN").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Cell::from("RUNS").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Cell::from("INTENT").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Cell::from("CREATED").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
    ])
    .bottom_margin(1);

    let sel_bg = app.palette.selected_bg;
    let sel_fg = app.palette.selected_fg;

    let rows: Vec<Row> = app
        .schedules
        .schedules
        .iter()
        .map(|s| {
            let sched_label = truncate(&format_schedule(&s.schedule), 20);
            let next = next_run_label(s);
            let runs = s.run_count.to_string();
            let intent = truncate(&s.intent, 40);
            let created = format_ts(s.created_at);
            Row::new(vec![
                Cell::from(sched_label).style(Style::default().fg(accent)),
                Cell::from(next),
                Cell::from(runs).style(Style::default().fg(dim)),
                Cell::from(intent),
                Cell::from(created).style(Style::default().fg(dim)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(22),
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Min(20),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(sel_bg)
                .fg(sel_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut app.schedules.table_state);
}
