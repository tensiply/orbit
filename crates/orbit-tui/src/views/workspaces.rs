use crate::app::App;
use orbit_core::plan::PlanStatus as CorePlanStatus;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

pub fn render(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let warning = app.palette.warning;
    let label = app.palette.label;

    if app.workspaces_reg.entries.is_empty() {
        f.render_widget(
            Paragraph::new(
                "No workspaces registered.\nRun `orbit workspace add <path>` to register one.\nPress [r] to refresh.",
            )
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" Workspaces ")),
            area,
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // ── Left panel: workspace list ────────────────────────────────────────────
    let selected_idx = app.workspaces_reg.selected;
    let ws_items: Vec<ListItem> = app
        .workspaces_reg
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let plan_count = app
                .plans
                .plans
                .iter()
                .filter(|p| p.scope.workspace.as_deref() == Some(entry.name.as_str()))
                .count();
            let running = app
                .plans
                .plans
                .iter()
                .filter(|p| {
                    p.scope.workspace.as_deref() == Some(entry.name.as_str())
                        && p.status == CorePlanStatus::Running
                })
                .count();

            let default_marker = if entry.is_default { " *" } else { "  " };
            let run_hint = if running > 0 {
                format!(", {} ▶", running)
            } else {
                String::new()
            };
            let line = format!(
                "{}{} ({}) — {} plans{}",
                default_marker, entry.name, entry.slug, plan_count, run_hint
            );

            let active = i == selected_idx;
            let style = if active {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else if running > 0 {
                Style::default().fg(label)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let ws_list = List::new(ws_items)
        .block(Block::default().borders(Borders::ALL).title(" Workspaces "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut list_state = ListState::default();
    list_state.select(Some(selected_idx));
    f.render_stateful_widget(ws_list, chunks[0], &mut list_state);

    // ── Right panel: details for selected workspace ───────────────────────────
    let Some(entry) = app.workspaces_reg.entries.get(selected_idx) else {
        return;
    };

    let ws_plans: Vec<_> = app
        .plans
        .plans
        .iter()
        .filter(|p| p.scope.workspace.as_deref() == Some(entry.name.as_str()))
        .collect();

    let total = ws_plans.len();
    let completed = ws_plans
        .iter()
        .filter(|p| p.status == CorePlanStatus::Completed)
        .count();
    let running = ws_plans
        .iter()
        .filter(|p| p.status == CorePlanStatus::Running)
        .count();
    let failed = ws_plans
        .iter()
        .filter(|p| p.status == CorePlanStatus::Failed)
        .count();
    let pending = total - completed - running - failed;

    let data_dir = orbit_core::data_paths::plans_dir_for(Some(&entry.slug));
    let data_parent = data_dir
        .parent()
        .unwrap_or(&data_dir)
        .to_string_lossy()
        .to_string();

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("  Name:   ", Style::default().fg(dim)),
            Span::styled(
                entry.name.clone(),
                Style::default().fg(label).add_modifier(Modifier::BOLD),
            ),
            if entry.is_default {
                Span::styled("  (default)", Style::default().fg(accent))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(vec![
            Span::styled("  Slug:   ", Style::default().fg(dim)),
            Span::raw(entry.slug.clone()),
        ]),
        Line::from(vec![
            Span::styled("  Path:   ", Style::default().fg(dim)),
            Span::raw(entry.ai_root.to_string_lossy().to_string()),
        ]),
        Line::from(vec![
            Span::styled("  Data:   ", Style::default().fg(dim)),
            Span::raw(data_parent),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Plans:  ", Style::default().fg(dim)),
            Span::styled(format!("{} total", total), Style::default()),
        ]),
    ];

    if completed > 0 {
        lines.push(Line::from(vec![
            Span::styled("           ✓ ", Style::default().fg(accent)),
            Span::raw(format!("{} completed", completed)),
        ]));
    }
    if running > 0 {
        lines.push(Line::from(vec![
            Span::styled("           ▶ ", Style::default().fg(accent)),
            Span::raw(format!("{} running", running)),
        ]));
    }
    if failed > 0 {
        lines.push(Line::from(vec![
            Span::styled("           ✗ ", Style::default().fg(warning)),
            Span::raw(format!("{} failed", failed)),
        ]));
    }
    if pending > 0 {
        lines.push(Line::from(vec![
            Span::styled("           · ", Style::default().fg(dim)),
            Span::raw(format!("{} pending/other", pending)),
        ]));
    }

    if total > 0 {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Recent plans:",
            Style::default().fg(dim),
        )));
        for p in ws_plans.iter().rev().take(5) {
            let icon = match p.status {
                CorePlanStatus::Completed => Span::styled("  ✓ ", Style::default().fg(accent)),
                CorePlanStatus::Failed => Span::styled("  ✗ ", Style::default().fg(warning)),
                CorePlanStatus::Running => Span::styled("  ▶ ", Style::default().fg(accent)),
                CorePlanStatus::Cancelled => Span::styled("  ⊘ ", Style::default().fg(dim)),
                _ => Span::styled("  · ", Style::default().fg(dim)),
            };
            let intent = p.intent.chars().take(60).collect::<String>();
            let ellipsis = if p.intent.len() > 60 { "…" } else { "" };
            lines.push(Line::from(vec![
                icon,
                Span::raw(format!("{}{}", intent, ellipsis)),
            ]));
        }
    }

    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", entry.name)),
        ),
        chunks[1],
    );
}
