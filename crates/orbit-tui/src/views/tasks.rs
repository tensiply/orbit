use crate::app::App;
use orbit_core::jira::JiraIssue;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table},
};

pub fn render(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim))
        .title(Span::styled(" Tasks ", Style::default().fg(dim)))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if !app.jira_enabled {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Jira plugin not enabled.",
                Style::default().fg(dim),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Run ", Style::default().fg(dim)),
                Span::styled("orbit plugins install jira", Style::default().fg(accent)),
                Span::styled(" to get started.", Style::default().fg(dim)),
            ]),
        ];
        f.render_widget(Paragraph::new(text), inner);
        return;
    }

    if app.tasks.loading {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Loading issues…",
                Style::default().fg(dim),
            )),
        ];
        f.render_widget(Paragraph::new(text), inner);
        return;
    }

    if let Some(err) = &app.tasks.error.clone() {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {err}"),
                Style::default().fg(dim),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Press ", Style::default().fg(dim)),
                Span::styled("[r]", Style::default().fg(accent)),
                Span::styled(" to retry.", Style::default().fg(dim)),
            ]),
        ];
        f.render_widget(Paragraph::new(text), inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    render_filter_bar(f, app, chunks[0]);
    render_table(f, app, chunks[1]);
}

fn render_filter_bar(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let orgs = app.tasks.orgs();
    let count = app.tasks.filtered_count();

    let mut spans = Vec::new();

    if orgs.is_empty() {
        spans.push(Span::styled(
            format!("  {} issue{}", count, if count == 1 { "" } else { "s" }),
            Style::default().fg(dim),
        ));
    } else {
        let filter_label = if app.tasks.org_filter_idx == 0 {
            "All".to_string()
        } else {
            orgs.get(app.tasks.org_filter_idx - 1)
                .cloned()
                .unwrap_or_else(|| "All".to_string())
        };

        spans.push(Span::styled(
            format!("  {filter_label}"),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("  {} issue{}", count, if count == 1 { "" } else { "s" }),
            Style::default().fg(dim),
        ));

        if orgs.len() > 1 {
            spans.push(Span::styled(
                "  [←→] org",
                Style::default().fg(dim),
            ));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_table(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let dim = app.palette.dim;
    let filtered = app.tasks.filtered_issues();

    if filtered.is_empty() {
        let msg = if app.tasks.issues.is_empty() {
            "  No active assigned issues found.  Press [r] to refresh."
        } else {
            "  No issues for this org."
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(dim))),
            area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("KEY").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
        Cell::from("TYPE").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
        Cell::from("SUMMARY").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
        Cell::from("STATUS").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
    ])
    .height(1);

    let rows: Vec<Row> = filtered.iter().map(|issue| issue_row(issue, dim)).collect();

    let sel_bg = app.palette.selected_bg;
    let sel_fg = app.palette.selected_fg;
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Min(20),
            Constraint::Length(20),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(sel_bg)
            .fg(sel_fg)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut app.tasks.table_state);
}

fn issue_row(issue: &JiraIssue, dim: Color) -> Row<'static> {
    let (pri_sym, pri_style) = priority_display(&issue.priority, dim);
    let (status_str, status_style) = status_display(&issue.status, &issue.status_color, dim);

    Row::new(vec![
        Cell::from(pri_sym).style(pri_style),
        Cell::from(issue.key.clone()).style(Style::default().fg(Color::Reset)),
        Cell::from(issue.issue_type.clone()).style(Style::default().fg(dim)),
        Cell::from(issue.summary.clone()).style(Style::default().fg(Color::Reset)),
        Cell::from(status_str).style(status_style),
    ])
    .height(1)
}

fn priority_display(priority: &str, dim: Color) -> (&'static str, Style) {
    let p = priority.to_lowercase();
    if p.contains("highest") || p.contains("critical") || p.contains("blocker") {
        ("↑↑", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    } else if p.contains("high") {
        ("↑ ", Style::default().fg(Color::Yellow))
    } else if p.contains("medium") || p.contains("normal") || p.contains("medio") {
        ("→ ", Style::default().fg(Color::Reset))
    } else if p.contains("low") {
        ("↓ ", Style::default().fg(dim))
    } else {
        ("·  ", Style::default().fg(dim))
    }
}

fn status_display(status: &str, color_name: &str, dim: Color) -> (String, Style) {
    let style = match color_name {
        "yellow"           => Style::default().fg(Color::Yellow),
        "green"            => Style::default().fg(Color::Green),
        "warm-red" | "red" => Style::default().fg(Color::Red),
        _                  => Style::default().fg(dim),
    };
    (status.to_string(), style)
}
