use crate::app::App;
use orbit_core::task::{OrbitTask, TaskPriority, TaskStatus};
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
        .padding(Padding::uniform(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.tasks.loading {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled("  Loading tasks…", Style::default().fg(dim))),
        ];
        f.render_widget(Paragraph::new(text), inner);
        return;
    }

    if let Some(err) = &app.tasks.error.clone() {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(format!("  {err}"), Style::default().fg(dim))),
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
    let sources = app.tasks.sources();
    let count = app.tasks.filtered_count();

    let mut spans = Vec::new();

    if sources.is_empty() {
        spans.push(Span::styled(
            format!("  {} task{}", count, if count == 1 { "" } else { "s" }),
            Style::default().fg(dim),
        ));
    } else {
        let filter_label = if app.tasks.source_filter_idx == 0 {
            "All".to_string()
        } else {
            sources
                .get(app.tasks.source_filter_idx - 1)
                .cloned()
                .unwrap_or_else(|| "All".to_string())
        };

        spans.push(Span::styled(
            format!("  {filter_label}"),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("  {} task{}", count, if count == 1 { "" } else { "s" }),
            Style::default().fg(dim),
        ));

        if sources.len() > 1 {
            spans.push(Span::styled("  [←→] source", Style::default().fg(dim)));
        }
    }

    spans.push(Span::styled(
        "  [r] refresh  [R] force-refresh  [↵] open",
        Style::default().fg(dim),
    ));

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_table(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let dim = app.palette.dim;
    let warning = app.palette.warning;
    let success = app.palette.success;
    let danger = app.palette.danger;
    let filtered = app.tasks.filtered_items();

    if filtered.is_empty() {
        let msg = if app.tasks.items.is_empty() {
            "  No tasks yet.  Run `orbit task add \"...\"` or press [R] to sync."
        } else {
            "  No tasks for this source."
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(dim))),
            area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from("").style(Style::default().fg(dim)),
        Cell::from("ID").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
        Cell::from("SRC").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
        Cell::from("TYPE").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
        Cell::from("TITLE").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
        Cell::from("STATUS").style(Style::default().fg(dim).add_modifier(Modifier::DIM)),
    ])
    .height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|task| task_row(task, dim, warning, success, danger))
        .collect();

    let sel_bg = app.palette.selected_bg;
    let sel_fg = app.palette.selected_fg;
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(14),
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

fn task_row(
    task: &OrbitTask,
    dim: Color,
    warning: Color,
    success: Color,
    danger: Color,
) -> Row<'static> {
    let (pri_sym, pri_style) = priority_display(&task.priority, dim, warning, danger);
    let (status_str, status_style) = status_display(&task.status, dim, warning, success, danger);

    let src = task.source.label();
    let ext_suffix = task
        .source
        .external_id()
        .map(|id| format!(" ({id})"))
        .unwrap_or_default();

    let type_str = task.task_type.clone().unwrap_or_else(|| "task".to_string());

    Row::new(vec![
        Cell::from(pri_sym).style(pri_style),
        Cell::from(task.id.clone()).style(Style::default().fg(Color::Reset)),
        Cell::from(src).style(Style::default().fg(dim)),
        Cell::from(type_str).style(Style::default().fg(dim)),
        Cell::from(format!("{}{}", task.title, ext_suffix))
            .style(Style::default().fg(Color::Reset)),
        Cell::from(status_str).style(status_style),
    ])
    .height(1)
}

fn priority_display(
    priority: &TaskPriority,
    dim: Color,
    warning: Color,
    danger: Color,
) -> (&'static str, Style) {
    match priority {
        TaskPriority::Critical => (
            "↑↑",
            Style::default().fg(danger).add_modifier(Modifier::BOLD),
        ),
        TaskPriority::High => ("↑ ", Style::default().fg(warning)),
        TaskPriority::Medium => ("→ ", Style::default().fg(Color::Reset)),
        TaskPriority::Low => ("↓ ", Style::default().fg(dim)),
    }
}

fn status_display(
    status: &TaskStatus,
    dim: Color,
    warning: Color,
    success: Color,
    danger: Color,
) -> (String, Style) {
    let style = match status {
        TaskStatus::Done => Style::default().fg(success),
        TaskStatus::InProgress => Style::default().fg(warning),
        TaskStatus::Blocked => Style::default().fg(danger),
        TaskStatus::Cancelled => Style::default().fg(dim),
        TaskStatus::Todo => Style::default().fg(Color::Reset),
    };
    (status.display().to_string(), style)
}
