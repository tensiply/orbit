use crate::app::App;
use orbit_core::plan::{NodeStatus, Plan, PlanStatus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table},
};

fn status_symbol(s: &PlanStatus) -> &'static str {
    match s {
        PlanStatus::Planning => "◌",
        PlanStatus::Running | PlanStatus::Replanning => "●",
        PlanStatus::Paused => "⏸",
        PlanStatus::Completed => "✓",
        PlanStatus::Failed => "✗",
        PlanStatus::Cancelled => "⊘",
    }
}

fn node_status_symbol(s: &NodeStatus) -> &'static str {
    match s {
        NodeStatus::Pending => "·",
        NodeStatus::Running => "●",
        NodeStatus::Completed => "✓",
        NodeStatus::Failed => "✗",
        NodeStatus::Skipped => "–",
        NodeStatus::AwaitingApproval => "⏸",
    }
}

fn read_log_tail(session_key: &str, n: usize) -> Option<String> {
    let path = std::env::temp_dir()
        .join("orbit-plan-nodes")
        .join(format!("{session_key}.log"));
    let content = std::fs::read_to_string(path).ok()?;
    if content.is_empty() {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(n);
    Some(lines[start..].join("\n"))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn format_ts(secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let elapsed = now.saturating_sub(secs);
    if elapsed < 60 {
        format!("{elapsed}s ago")
    } else if elapsed < 3600 {
        format!("{}m ago", elapsed / 60)
    } else {
        format!("{}h ago", elapsed / 3600)
    }
}

pub fn render(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let label = app.palette.label;
    let success = app.palette.success;
    let warning = app.palette.warning;
    let error_color = app.palette.danger;

    // Split: plans list (top) + node detail (bottom)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // ── Plans list ────────────────────────────────────────────────────────────

    let plan_count = app.plans.plans.len();
    let title = if plan_count == 0 {
        " Plans — none ".to_string()
    } else {
        format!(" Plans — {plan_count} ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim))
        .title(Span::styled(title, Style::default().fg(dim)))
        .padding(Padding::horizontal(1));

    if app.plans.plans.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No plans found. Run: orbit plan \"<intent>\"",
                Style::default().fg(dim),
            )),
        ])
        .block(block);
        f.render_widget(msg, chunks[0]);

        // Empty bottom panel
        let detail_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(dim))
            .title(Span::styled(" Nodes ", Style::default().fg(dim)));
        f.render_widget(detail_block, chunks[1]);
        return;
    }

    let header = Row::new(vec![
        Cell::from("").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Cell::from("STATUS").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Cell::from("NODES").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Cell::from("INTENT").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
        Cell::from("CREATED").style(Style::default().fg(label).add_modifier(Modifier::BOLD)),
    ])
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .plans
        .plans
        .iter()
        .map(|p| {
            let sym = status_symbol(&p.status);
            let sym_style = match p.status {
                PlanStatus::Completed => Style::default().fg(success),
                PlanStatus::Failed => Style::default().fg(error_color),
                PlanStatus::Running | PlanStatus::Planning | PlanStatus::Replanning => {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                }
                PlanStatus::Paused => Style::default().fg(dim).add_modifier(Modifier::ITALIC),
                PlanStatus::Cancelled => Style::default().fg(dim),
            };
            let status_label = format!("{:?}", p.status);
            let node_count = p.nodes.len().to_string();
            let intent = truncate(&p.intent, 50);
            let created = format_ts(p.created_at);
            Row::new(vec![
                Cell::from(sym).style(sym_style),
                Cell::from(status_label),
                Cell::from(node_count),
                Cell::from(intent),
                Cell::from(created).style(Style::default().fg(dim)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(16),
        Constraint::Length(6),
        Constraint::Min(20),
        Constraint::Length(10),
    ];

    let sel_bg = app.palette.selected_bg;
    let sel_fg = app.palette.selected_fg;
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

    f.render_stateful_widget(table, chunks[0], &mut app.plans.table_state);

    // ── Node detail ───────────────────────────────────────────────────────────

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim))
        .title(Span::styled(" Nodes ", Style::default().fg(dim)))
        .padding(Padding::horizontal(1));

    let selected_plan: Option<&Plan> = app.plans.plans.get(app.plans.selected);
    match selected_plan {
        None => {
            f.render_widget(detail_block, chunks[1]);
        }
        Some(plan) => {
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(vec![
                Span::styled("Plan  ", Style::default().fg(dim)),
                Span::raw(plan.id.clone()),
                Span::styled("   ", Style::default()),
                Span::styled(
                    format!("{:?}", plan.status),
                    match plan.status {
                        PlanStatus::Completed => Style::default().fg(success),
                        PlanStatus::Failed => Style::default().fg(error_color),
                        _ => Style::default().fg(accent),
                    },
                ),
            ]));

            if plan.replan_count > 0 {
                lines.push(Line::from(Span::styled(
                    format!("  {} replan(s)", plan.replan_count),
                    Style::default().fg(warning),
                )));
            }

            lines.push(Line::from(""));

            let total_cost: f64 = plan
                .nodes
                .iter()
                .filter_map(|n| n.token_usage.as_ref())
                .map(|u| u.estimated_cost_usd)
                .sum();

            for node in &plan.nodes {
                let sym = node_status_symbol(&node.status);
                let sym_style = match node.status {
                    NodeStatus::Completed => Style::default().fg(success),
                    NodeStatus::Failed => Style::default().fg(error_color),
                    NodeStatus::Running => Style::default().fg(accent).add_modifier(Modifier::BOLD),
                    NodeStatus::Pending => Style::default().fg(dim),
                    _ => Style::default().fg(warning),
                };
                let cost_span = if let Some(u) = &node.token_usage {
                    Span::styled(
                        format!("  ~${:.4}", u.estimated_cost_usd),
                        Style::default().fg(dim),
                    )
                } else {
                    Span::raw("")
                };
                let repo_span = if let Some(ref s) = node.scope_override {
                    if let Some(ref r) = s.repository {
                        Span::styled(format!(" [→{r}]"), Style::default().fg(warning))
                    } else {
                        Span::raw("")
                    }
                } else {
                    Span::raw("")
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {sym} "), sym_style),
                    Span::raw(truncate(&node.label, 40)),
                    Span::styled(
                        format!("  [{:?}]", node.task_type),
                        Style::default().fg(dim),
                    ),
                    cost_span,
                    repo_span,
                ]));

                let plan_suffix = plan.id.trim_start_matches("plan_");
                let session_key = format!("orbit-plan-{plan_suffix}-{}", node.id);

                if node.status == NodeStatus::Running && node.session_id.is_some() {
                    lines.push(Line::from(Span::styled(
                        format!("       tmux attach -t {session_key}"),
                        Style::default().fg(dim),
                    )));
                }

                // Log preview for Running / Completed / Failed nodes
                if matches!(
                    node.status,
                    NodeStatus::Running | NodeStatus::Completed | NodeStatus::Failed
                ) && let Some(preview) = read_log_tail(&session_key, 4)
                {
                    for log_line in preview.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("       │ {log_line}"),
                            Style::default().fg(dim),
                        )));
                    }
                    lines.push(Line::from(""));
                }
            }

            if total_cost > 0.0 {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("  Est. cost  ", Style::default().fg(dim)),
                    Span::styled(format!("~${total_cost:.4}"), Style::default().fg(label)),
                ]));
            }

            f.render_widget(Paragraph::new(lines).block(detail_block), chunks[1]);
        }
    }
}
