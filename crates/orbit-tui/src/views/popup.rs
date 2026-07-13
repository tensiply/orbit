use crate::app::{
    AddMcpField, AddMcpState, FieldSelectState, LaunchField, McpScope, ShareDialogState,
    ShareRole, ShareStatus, WriteJiraState,
};
use crate::mcp::McpEntry;
use crate::theme::Palette;
use crate::views::adf;
use crate::widget::TextInput;
use orbit_core::{jira::JiraIssueDetail, session::Session};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

// ── layout helper ─────────────────────────────────────────────────────────────

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let h_margin = area.width.saturating_sub(width) / 2;
    let v_margin = area.height.saturating_sub(height) / 2;
    Rect {
        x: area.x + h_margin,
        y: area.y + v_margin,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn render_popup(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line>, accent: Color) {
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(lines), inner);
}

// ── help popup ────────────────────────────────────────────────────────────────

pub fn render_help(f: &mut Frame, area: Rect, palette: &Palette) {
    let popup_area = centered_rect(52, 28, area);
    let accent = palette.accent;
    let dim = palette.dim;
    let label = palette.label;

    let k = |s: &'static str| Span::styled(s, Style::default().fg(accent));
    let d = |s: &'static str| Span::raw(s);
    let section = move |s: &'static str| {
        Line::from(Span::styled(
            s,
            Style::default().fg(label).add_modifier(Modifier::BOLD),
        ))
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![k("  Tab / [1-3]  "), d("  Switch tabs")]),
        Line::from(""),
        section("  Sessions tab:"),
        Line::from(vec![k("  ↑↓ / jk     "), d("  Navigate sessions")]),
        Line::from(vec![k("  a / Enter   "), d("  Attach to session")]),
        Line::from(vec![k("  K           "), d("  Kill session (confirm)")]),
        Line::from(vec![k("  d           "), d("  Session details")]),
        Line::from(vec![k("  c           "), d("  Clean dead sessions")]),
        Line::from(vec![k("  r           "), d("  Refresh")]),
        Line::from(vec![k("  q / Esc     "), d("  Quit")]),
        Line::from(""),
        section("  Launch tab:"),
        Line::from(vec![k("  ↑↓          "), d("  Move between fields")]),
        Line::from(vec![k("  ←→          "), d("  Cycle engine")]),
        Line::from(vec![k("  Space        "), d("  Toggle no-tmux")]),
        Line::from(vec![k("  Enter        "), d("  Confirm / Launch")]),
        Line::from(vec![k("  Esc          "), d("  Back to Sessions")]),
        Line::from(""),
        section("  System tab:"),
        Line::from(vec![k("  ↑↓ / jk     "), d("  Navigate MCP list")]),
        Line::from(vec![k("  a           "), d("  Add MCP server")]),
        Line::from(vec![k("  x           "), d("  Remove selected MCP")]),
        Line::from(vec![k("  s           "), d("  Toggle daemon")]),
        Line::from(vec![k("  r           "), d("  Refresh")]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            Style::default().fg(dim),
        )),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Keybindings", lines, accent);
}

// ── confirm kill popup ────────────────────────────────────────────────────────

pub fn render_confirm_kill(f: &mut Frame, area: Rect, session: Session, palette: &Palette) {
    let popup_area = centered_rect(54, 9, area);
    let dim = palette.dim;
    let warning = palette.warning;
    let danger = palette.danger;

    let alive = if session.is_running() {
        "alive"
    } else {
        "dead"
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Kill session "),
            Span::styled(session.id.clone(), Style::default().fg(warning)),
            Span::raw("?"),
        ]),
        Line::from(vec![Span::styled(
            format!(
                "  {} │ {}  ({})",
                session.engine,
                session.scope_label(),
                alive
            ),
            Style::default().fg(dim),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("       "),
            Span::styled(
                "[y] Confirm",
                Style::default().fg(danger).add_modifier(Modifier::BOLD),
            ),
            Span::raw("       "),
            Span::styled("[Esc/n] Cancel", Style::default().fg(dim)),
        ]),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Kill Session", lines, palette.accent);
}

// ── session details popup ─────────────────────────────────────────────────────

pub fn render_details(f: &mut Frame, area: Rect, session: Session, palette: &Palette) {
    let popup_area = centered_rect(62, 14, area);
    let accent = palette.accent;
    let dim = palette.dim;
    let success = palette.success;
    let danger = palette.danger;

    let alive = session.is_running();
    let status_span = if alive {
        Span::styled("● alive", Style::default().fg(success))
    } else {
        Span::styled("○ dead", Style::default().fg(dim))
    };

    let k = |s: &str| Span::styled(format!("{:<12}", s), Style::default().fg(dim));

    let work_dir = session.work_dir.display().to_string();
    let tmux = session
        .tmux_session
        .as_deref()
        .unwrap_or("(none)")
        .to_string();

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            k("ID:"),
            Span::raw(session.id.clone()),
        ]),
        Line::from(vec![
            Span::raw("  "),
            k("PID:"),
            Span::raw(session.pid.to_string()),
            Span::styled("   Status: ", Style::default().fg(dim)),
            status_span,
        ]),
        Line::from(vec![
            Span::raw("  "),
            k("Engine:"),
            Span::raw(session.engine.clone()),
        ]),
        Line::from(vec![
            Span::raw("  "),
            k("Scope:"),
            Span::raw(session.scope_label()),
        ]),
        Line::from(vec![Span::raw("  "), k("Dir:"), Span::raw(work_dir)]),
        Line::from(vec![Span::raw("  "), k("Tmux:"), Span::raw(tmux)]),
        Line::from(vec![
            Span::raw("  "),
            k("Started:"),
            Span::raw(session.started_ago()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[a]", Style::default().fg(accent)),
            Span::raw(" Attach   "),
            Span::styled("[K]", Style::default().fg(danger)),
            Span::raw(" Kill   "),
            Span::styled("[Esc]", Style::default().fg(dim)),
            Span::raw(" Close"),
        ]),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Session Details", lines, accent);
}

// ── add mcp popup ─────────────────────────────────────────────────────────────

pub fn render_add_mcp(
    f: &mut Frame,
    area: Rect,
    state: &AddMcpState,
    _default_tenant: &str,
    palette: &Palette,
) {
    let popup_area = centered_rect(64, 19, area);
    let accent = palette.accent;
    let dim = palette.dim;

    let project_active = state.scope.needs_project();
    let repo_active = state.scope.needs_repo();

    let lines = vec![
        Line::from(""),
        mcp_input_line(
            "Name:",
            &state.name,
            state.focused == AddMcpField::Name,
            accent,
            dim,
        ),
        mcp_input_line(
            "Command:",
            &state.command,
            state.focused == AddMcpField::Command,
            accent,
            dim,
        ),
        mcp_input_line(
            "Args:",
            &state.args,
            state.focused == AddMcpField::Args,
            accent,
            dim,
        ),
        mcp_input_line(
            "Env:",
            &state.env,
            state.focused == AddMcpField::Env,
            accent,
            dim,
        ),
        Line::from(vec![
            Span::raw("            "),
            Span::styled("(comma-sep: KEY=VALUE,KEY2=V2)", Style::default().fg(dim)),
        ]),
        Line::from(""),
        mcp_scope_line(
            state.focused == AddMcpField::Scope,
            state.scope,
            accent,
            dim,
        ),
        Line::from(""),
        mcp_input_line_opt(
            "Project:",
            &state.project_name,
            state.focused == AddMcpField::ProjectName,
            project_active,
            accent,
            dim,
        ),
        mcp_input_line_opt(
            "Repo:",
            &state.repo_name,
            state.focused == AddMcpField::RepoName,
            repo_active,
            accent,
            dim,
        ),
        Line::from(""),
        mcp_confirm_line(state.focused == AddMcpField::Confirm, accent, dim),
        Line::from(""),
        Line::from(Span::styled(
            "  ↑↓ fields  ←→ scope  Enter next/confirm  Esc cancel",
            Style::default().fg(dim),
        )),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Add MCP Server", lines, accent);
}

fn mcp_input_line(
    label: &str,
    input: &TextInput,
    focused: bool,
    accent: Color,
    dim: Color,
) -> Line<'static> {
    let label_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(dim)
    };
    let val_style = if focused {
        Style::default()
    } else {
        Style::default().fg(dim)
    };
    let text = input.display(focused);
    let padded = pad42(&text);
    Line::from(vec![
        Span::styled(format!("  {:<10}", label), label_style),
        Span::styled("[".to_string(), Style::default().fg(dim)),
        Span::styled(padded, val_style),
        Span::styled("]".to_string(), Style::default().fg(dim)),
    ])
}

fn mcp_scope_line(focused: bool, scope: McpScope, accent: Color, dim: Color) -> Line<'static> {
    let label_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(dim)
    };
    let scopes = [
        McpScope::Global,
        McpScope::Tenant,
        McpScope::Project,
        McpScope::Repo,
    ];
    let mut spans = vec![Span::styled("  Scope:   ".to_string(), label_style)];
    for (i, s) in scopes.iter().enumerate() {
        let active = *s == scope;
        let text = if active {
            format!("● {}", s.label())
        } else {
            format!("○ {}", s.label())
        };
        let style = if active {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(dim)
        };
        spans.push(Span::styled(text, style));
        if i < scopes.len() - 1 {
            spans.push(Span::styled("  ".to_string(), Style::default()));
        }
    }
    spans.push(Span::styled("  [←→]".to_string(), Style::default().fg(dim)));
    Line::from(spans)
}

fn mcp_input_line_opt(
    label: &str,
    input: &TextInput,
    focused: bool,
    active: bool,
    accent: Color,
    dim: Color,
) -> Line<'static> {
    let dim_style = Style::default().fg(dim);
    let label_style = if focused && active {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        dim_style
    };
    let val_style = if active { Style::default() } else { dim_style };
    let text = if active {
        input.display(focused)
    } else {
        "  (not used for this scope)".to_string()
    };
    let padded = pad42(&text);
    Line::from(vec![
        Span::styled(format!("  {:<10}", label), label_style),
        Span::styled("[".to_string(), dim_style),
        Span::styled(padded, val_style),
        Span::styled("]".to_string(), dim_style),
    ])
}

fn mcp_confirm_line(focused: bool, accent: Color, dim: Color) -> Line<'static> {
    if focused {
        Line::from(vec![
            Span::styled(
                "  ▶ ".to_string(),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "[ Add Server ]".to_string(),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ← press Enter".to_string(), Style::default().fg(dim)),
        ])
    } else {
        Line::from(vec![
            Span::raw("    ".to_string()),
            Span::styled("[ Add Server ]".to_string(), Style::default().fg(dim)),
        ])
    }
}

// ── field select popup ────────────────────────────────────────────────────────

pub fn render_field_select(f: &mut Frame, area: Rect, state: &FieldSelectState, palette: &Palette) {
    let accent = palette.accent;
    let dim = palette.dim;
    let field_name = match state.field {
        LaunchField::Tenant => "Tenant",
        LaunchField::Project => "Project",
        LaunchField::Repository => "Repository",
        _ => "Select",
    };
    let title = format!("Select {field_name}");

    let filtered = state.filtered_options();
    let n_opts = filtered.len();
    let height = (n_opts as u16 + 6).clamp(8, 20);
    let popup_area = centered_rect(44, height, area);

    let mut lines: Vec<Line> = vec![Line::from("")];

    let warning = palette.warning;
    let filter_display = if state.filter.is_empty() {
        Span::styled("  (type to filter)", Style::default().fg(dim))
    } else {
        Span::styled(
            format!("  filter: {}", state.filter),
            Style::default().fg(warning),
        )
    };
    lines.push(Line::from(filter_display));
    lines.push(Line::from(""));

    for (i, opt) in filtered.iter().enumerate() {
        let selected = i == state.cursor;
        let (bullet, style) = if selected {
            (
                "▶ ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )
        } else {
            ("  ", Style::default().fg(Color::Reset))
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {bullet}"), style),
            Span::styled(opt.to_string(), style),
        ]));
    }

    if n_opts == 0 {
        lines.push(Line::from(Span::styled(
            "  (no options found)",
            Style::default().fg(dim),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ↑↓ navigate  Enter select  Esc cancel",
        Style::default().fg(dim),
    )));

    render_popup(f, popup_area, &title, lines, accent);
}

fn pad42(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 42 {
        chars[..42].iter().collect()
    } else {
        let mut result = s.to_string();
        for _ in 0..(42 - chars.len()) {
            result.push(' ');
        }
        result
    }
}

// ── confirm remove mcp popup ──────────────────────────────────────────────────

pub fn render_confirm_remove_mcp(f: &mut Frame, area: Rect, entry: &McpEntry, palette: &Palette) {
    let popup_area = centered_rect(56, 9, area);
    let dim = palette.dim;
    let warning = palette.warning;
    let danger = palette.danger;

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Remove "),
            Span::styled(format!("\"{}\"", entry.name), Style::default().fg(warning)),
            Span::styled(format!(" ({})?", entry.scope), Style::default().fg(dim)),
        ]),
        Line::from(Span::styled(
            format!("  {}", entry.command_display),
            Style::default().fg(dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("        "),
            Span::styled(
                "[y] Remove".to_string(),
                Style::default().fg(danger).add_modifier(Modifier::BOLD),
            ),
            Span::raw("        "),
            Span::styled("[Esc/n] Cancel".to_string(), Style::default().fg(dim)),
        ]),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Remove MCP", lines, palette.accent);
}

// ── task details loading / error ──────────────────────────────────────────────

pub fn render_task_details_loading(f: &mut Frame, area: Rect, palette: &Palette) {
    let popup_area = centered_rect(50, 5, area);
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Fetching issue details…",
            Style::default().fg(palette.dim),
        )),
        Line::from(""),
    ];
    render_popup(f, popup_area, "Loading", lines, palette.accent);
}

pub fn render_task_details_error(f: &mut Frame, area: Rect, msg: &str, palette: &Palette) {
    let popup_area = centered_rect(60, 7, area);
    let dim = palette.dim;
    let danger = palette.danger;
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {msg}"),
            Style::default().fg(danger),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close.",
            Style::default().fg(dim),
        )),
        Line::from(""),
    ];
    render_popup(f, popup_area, "Error", lines, palette.accent);
}

pub fn render_task_details(f: &mut Frame, area: Rect, detail: &JiraIssueDetail, palette: &Palette) {
    let w = (area.width as f32 * 0.85) as u16;
    let h = (area.height as f32 * 0.85) as u16;
    let popup_area = centered_rect(w, h, area);
    let accent = palette.accent;
    let dim = palette.dim;

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            format!(" {} ", detail.key),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(inner);

    render_detail_meta(f, detail, chunks[0], accent, dim);
    render_detail_body(f, detail, chunks[1], accent, dim);
}

fn render_detail_meta(f: &mut Frame, d: &JiraIssueDetail, area: Rect, accent: Color, dim: Color) {
    let k = |s: &str| Span::styled(format!("{:<12}", s), Style::default().fg(accent));

    let status_style = match d.status_color.as_str() {
        "yellow" => Style::default().fg(Color::Yellow),
        "green" => Style::default().fg(Color::Green),
        "warm-red" | "red" => Style::default().fg(Color::Red),
        _ => Style::default().fg(dim),
    };

    let sp_str = match d.story_points {
        Some(pts) if pts.fract() == 0.0 => format!("{} pts", pts as u32),
        Some(pts) => format!("{pts} pts"),
        None => "—".to_string(),
    };

    let due_str = if d.due_date.is_empty() {
        "—".to_string()
    } else {
        d.due_date.clone()
    };
    let sprint_str = if d.sprint.is_empty() {
        "—".to_string()
    } else {
        d.sprint.clone()
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            k("Summary:"),
            Span::raw(d.summary.clone()),
        ]),
        Line::from(vec![
            Span::raw("  "),
            k("Status:"),
            Span::styled(d.status.clone(), status_style),
            Span::raw("   Priority: "),
            Span::raw(d.priority.clone()),
            Span::raw("   Type: "),
            Span::raw(d.issue_type.clone()),
            Span::raw("   Points: "),
            Span::raw(sp_str),
        ]),
        Line::from(vec![
            Span::raw("  "),
            k("Assignee:"),
            Span::raw(d.assignee.clone()),
            Span::raw("   Reporter: "),
            Span::raw(d.reporter.clone()),
        ]),
        Line::from(vec![
            Span::raw("  "),
            k("Sprint:"),
            Span::raw(sprint_str),
            Span::raw("   Due: "),
            Span::raw(due_str),
        ]),
        Line::from(vec![
            Span::raw("  "),
            k("Created:"),
            Span::raw(d.created.clone()),
            Span::raw("   Updated: "),
            Span::raw(d.updated.clone()),
        ]),
        Line::from(Span::styled(
            "─".repeat(area.width as usize),
            Style::default().fg(dim),
        )),
    ];

    f.render_widget(Paragraph::new(lines), area);
}

fn render_detail_body(f: &mut Frame, d: &JiraIssueDetail, area: Rect, accent: Color, dim: Color) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);
    let content_area = chunks[1];

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        "Description",
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if let Some(adf_val) = &d.description_adf {
        let adf_lines = adf::render(adf_val, dim);
        if adf_lines.is_empty() || adf_lines.iter().all(|l| l.spans.is_empty()) {
            lines.push(Line::from(Span::styled(
                "(no description)",
                Style::default().fg(dim),
            )));
        } else {
            lines.extend(adf_lines);
        }
    } else if d.description.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no description)",
            Style::default().fg(dim),
        )));
    } else {
        for text_line in d.description.lines() {
            lines.push(Line::from(Span::raw(text_line.to_string())));
        }
    }

    if !d.comments.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Comments ({})", d.comments.len()),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
        for comment in &d.comments {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", comment.author),
                    Style::default()
                        .fg(Color::Reset)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(comment.created.clone(), Style::default().fg(dim)),
            ]));
            lines.push(Line::from(""));
            for text_line in comment.body.lines() {
                lines.push(Line::from(Span::raw(text_line.to_string())));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("[c]", Style::default().fg(accent)),
        Span::styled(" add comment  ", Style::default().fg(dim)),
        Span::styled("[e]", Style::default().fg(accent)),
        Span::styled(" open in browser  ", Style::default().fg(dim)),
        Span::styled("[Esc]", Style::default().fg(dim)),
        Span::styled(" close", Style::default().fg(dim)),
    ]));

    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        content_area,
    );
}

// ── write comment popup ───────────────────────────────────────────────────────

pub fn render_add_comment(f: &mut Frame, area: Rect, state: &WriteJiraState, palette: &Palette) {
    render_write_popup(
        f,
        area,
        state,
        "Add Comment",
        "Comment",
        palette.accent,
        palette.dim,
    );
}

// ── share dialog popup ────────────────────────────────────────────────────────

pub fn render_share_dialog(
    f: &mut Frame,
    area: Rect,
    state: &ShareDialogState,
    palette: &Palette,
) {
    let popup_area = centered_rect(52, 12, area);
    f.render_widget(Clear, popup_area);

    let accent = palette.accent;
    let dim = palette.dim;

    let role_str = match state.role {
        ShareRole::Observer => "● Observer  ○ Contributor",
        ShareRole::Contributor => "○ Observer  ● Contributor",
    };

    let status_str = match &state.status {
        ShareStatus::Idle => "Press Enter to start sharing".to_string(),
        ShareStatus::Active { port } => format!("Sharing on port {} (mDNS active)", port),
        ShareStatus::Error(e) => format!("Error: {}", e),
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Role: ", Style::default().fg(dim)),
            Span::raw(role_str),
            Span::styled("  [←→]", Style::default().fg(dim)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Port: ", Style::default().fg(dim)),
            Span::raw(state.port_input.as_str().to_string()),
        ]),
        Line::from(vec![
            Span::styled("  Name: ", Style::default().fg(dim)),
            Span::raw(state.name_input.as_str().to_string()),
        ]),
        Line::from(""),
        Line::from(vec![Span::raw("  "), Span::raw(status_str)]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Enter]", Style::default().fg(accent)),
            Span::raw(" start/stop  "),
            Span::styled("[Esc]", Style::default().fg(dim)),
            Span::raw(" close"),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            " Share via LAN ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);
    f.render_widget(Paragraph::new(lines), inner);
}

fn render_write_popup(
    f: &mut Frame,
    area: Rect,
    state: &WriteJiraState,
    title: &str,
    field_label: &str,
    accent: Color,
    dim: Color,
) {
    let popup_area = centered_rect(70, 9, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            format!(" {} — {} ", title, state.key),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let input_text = state.input.display(true);
    let dim_style = Style::default().fg(dim);

    let lines = vec![Line::from("")];

    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(Paragraph::new(lines), input_chunks[0]);

    let label_and_box = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(16), Constraint::Min(10)])
        .split(input_chunks[1]);

    let label_area = ratatui::layout::Rect {
        y: label_and_box[0].y + 1,
        height: 1,
        ..label_and_box[0]
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  {field_label}:"),
            Style::default().fg(dim),
        )),
        label_area,
    );

    let box_area = ratatui::layout::Rect {
        width: label_and_box[1].width.min(50),
        ..label_and_box[1]
    };
    let box_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let box_inner = box_block.inner(box_area);
    f.render_widget(box_block, box_area);
    f.render_widget(Paragraph::new(Span::raw(input_text)), box_inner);

    let hint_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[↵]", Style::default().fg(accent)),
            Span::styled(" submit  ", dim_style),
            Span::styled("[Esc]", Style::default().fg(accent)),
            Span::styled(" cancel", dim_style),
        ]),
    ];
    f.render_widget(Paragraph::new(hint_lines), input_chunks[2]);
}
