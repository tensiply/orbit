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

    if app.scopes.scopes.is_empty() {
        f.render_widget(
            Paragraph::new("No plans found across any scope.\nPress [r] to refresh.")
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title(" Scopes ")),
            area,
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // ── Left panel: scope list ────────────────────────────────────────────────
    let selected_scope = app.scopes.selected_scope().map(str::to_string);
    let scope_items: Vec<ListItem> = app
        .scopes
        .scopes
        .iter()
        .map(|key| {
            let plans_in_scope: Vec<_> = app
                .plans
                .plans
                .iter()
                .filter(|p| &p.scope.scope_key() == key)
                .collect();
            let completed = plans_in_scope
                .iter()
                .filter(|p| p.status == CorePlanStatus::Completed)
                .count();
            let running = plans_in_scope
                .iter()
                .filter(|p| p.status == CorePlanStatus::Running)
                .count();
            let failed = plans_in_scope
                .iter()
                .filter(|p| p.status == CorePlanStatus::Failed)
                .count();
            let total = plans_in_scope.len();

            let active = selected_scope.as_deref() == Some(key.as_str());
            let label = if key.trim_matches('/').is_empty() {
                "(global)".to_string()
            } else {
                // Show only the last component of the path for brevity
                key.split('/').rfind(|s| !s.is_empty()).unwrap_or(key).to_string()
            };
            let summary = format!(
                " {} ─ {}/{} ok{}",
                label,
                completed,
                total,
                if running > 0 {
                    format!(", {} ▶", running)
                } else {
                    String::new()
                }
            );
            let style = if active {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else if failed > 0 {
                Style::default().fg(warning)
            } else {
                Style::default()
            };
            ListItem::new(summary).style(style)
        })
        .collect();

    let scope_list = List::new(scope_items)
        .block(Block::default().borders(Borders::ALL).title(" Scopes "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut list_state = ListState::default();
    list_state.select(Some(app.scopes.selected));
    f.render_stateful_widget(scope_list, chunks[0], &mut list_state);

    // ── Right panel: plans for selected scope ─────────────────────────────────
    let scope_key = selected_scope.unwrap_or_default();
    let full_scope = app
        .scopes
        .scopes
        .get(app.scopes.selected)
        .cloned()
        .unwrap_or_default();
    let scope_plans: Vec<_> = app
        .plans
        .plans
        .iter()
        .filter(|p| p.scope.scope_key() == full_scope)
        .collect();

    let plan_lines: Vec<Line> = if scope_plans.is_empty() {
        vec![Line::from(Span::styled(
            "  No plans in this scope.",
            Style::default().fg(dim),
        ))]
    } else {
        scope_plans
            .iter()
            .map(|p| {
                let icon = match p.status {
                    CorePlanStatus::Completed => Span::styled("✓ ", Style::default().fg(accent)),
                    CorePlanStatus::Failed => Span::styled("✗ ", Style::default().fg(warning)),
                    CorePlanStatus::Running => Span::styled("▶ ", Style::default().fg(accent)),
                    CorePlanStatus::Cancelled => Span::styled("⊘ ", Style::default().fg(dim)),
                    _ => Span::styled("· ", Style::default().fg(dim)),
                };
                let cost_str = {
                    let total: f64 = p
                        .nodes
                        .iter()
                        .filter_map(|n| n.token_usage.as_ref())
                        .map(|u| u.estimated_cost_usd)
                        .sum();
                    if total > 0.0 {
                        format!("  ~${total:.4}")
                    } else {
                        String::new()
                    }
                };
                let id_span = Span::styled(
                    format!("{} ", p.id),
                    Style::default().add_modifier(Modifier::DIM),
                );
                let intent_span =
                    Span::raw(format!("[{}n]{} — {}", p.nodes.len(), cost_str, p.intent));
                Line::from(vec![icon, id_span, intent_span])
            })
            .collect()
    };

    let title = if full_scope.is_empty() {
        " Plans (global) ".to_string()
    } else {
        format!(" Plans: {} ", full_scope)
    };

    f.render_widget(
        Paragraph::new(plan_lines)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false }),
        chunks[1],
    );

    // ── Footer hint ───────────────────────────────────────────────────────────
    let _ = (dim, scope_key); // suppress unused warnings
}
