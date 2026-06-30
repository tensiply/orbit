use crate::app::{AddMcpField, AddMcpState, FieldSelectState, LaunchField, McpScope};
use crate::mcp::McpEntry;
use crate::widget::TextInput;
use orbit_core::session::Session;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
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

fn render_popup(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line>) {
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(lines), inner);
}

// ── help popup ────────────────────────────────────────────────────────────────

pub fn render_help(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(52, 28, area);

    let k = |s: &'static str| Span::styled(s, Style::default().fg(Color::Cyan));
    let d = |s: &'static str| Span::raw(s);
    let section = |s: &'static str| {
        Line::from(Span::styled(
            s,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
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
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Keybindings", lines);
}

// ── confirm kill popup ────────────────────────────────────────────────────────

pub fn render_confirm_kill(f: &mut Frame, area: Rect, session: Session) {
    let popup_area = centered_rect(54, 9, area);

    let alive = if session.is_running() {
        "alive"
    } else {
        "dead"
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Kill session "),
            Span::styled(session.id.clone(), Style::default().fg(Color::Yellow)),
            Span::raw("?"),
        ]),
        Line::from(vec![Span::styled(
            format!(
                "  {} │ {}  ({})",
                session.engine,
                session.scope_label(),
                alive
            ),
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("       "),
            Span::styled(
                "[y] Confirm",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("       "),
            Span::styled("[Esc/n] Cancel", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Kill Session", lines);
}

// ── session details popup ─────────────────────────────────────────────────────

pub fn render_details(f: &mut Frame, area: Rect, session: Session) {
    let popup_area = centered_rect(62, 14, area);

    let alive = session.is_running();
    let status_span = if alive {
        Span::styled("● alive", Style::default().fg(Color::Green))
    } else {
        Span::styled("○ dead", Style::default().fg(Color::DarkGray))
    };

    let k = |s: &str| Span::styled(format!("{:<12}", s), Style::default().fg(Color::DarkGray));

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
            Span::styled("   Status: ", Style::default().fg(Color::DarkGray)),
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
            Span::styled("[a]", Style::default().fg(Color::Cyan)),
            Span::raw(" Attach   "),
            Span::styled("[K]", Style::default().fg(Color::Red)),
            Span::raw(" Kill   "),
            Span::styled("[Esc]", Style::default().fg(Color::DarkGray)),
            Span::raw(" Close"),
        ]),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Session Details", lines);
}

// ── add mcp popup ─────────────────────────────────────────────────────────────

pub fn render_add_mcp(f: &mut Frame, area: Rect, state: &AddMcpState, _default_tenant: &str) {
    let popup_area = centered_rect(64, 19, area);

    let project_active = state.scope.needs_project();
    let repo_active = state.scope.needs_repo();

    let lines = vec![
        Line::from(""),
        mcp_input_line("Name:", &state.name, state.focused == AddMcpField::Name),
        mcp_input_line(
            "Command:",
            &state.command,
            state.focused == AddMcpField::Command,
        ),
        mcp_input_line("Args:", &state.args, state.focused == AddMcpField::Args),
        mcp_input_line("Env:", &state.env, state.focused == AddMcpField::Env),
        Line::from(vec![
            Span::raw("            "),
            Span::styled(
                "(comma-sep: KEY=VALUE,KEY2=V2)",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(""),
        mcp_scope_line(state.focused == AddMcpField::Scope, state.scope),
        Line::from(""),
        mcp_input_line_opt(
            "Project:",
            &state.project_name,
            state.focused == AddMcpField::ProjectName,
            project_active,
        ),
        mcp_input_line_opt(
            "Repo:",
            &state.repo_name,
            state.focused == AddMcpField::RepoName,
            repo_active,
        ),
        Line::from(""),
        mcp_confirm_line(state.focused == AddMcpField::Confirm),
        Line::from(""),
        Line::from(Span::styled(
            "  ↑↓ fields  ←→ scope  Enter next/confirm  Esc cancel",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Add MCP Server", lines);
}

fn mcp_input_line(label: &str, input: &TextInput, focused: bool) -> Line<'static> {
    let label_style = if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let val_style = if focused {
        Style::default()
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let text = input.display(focused);
    let padded = pad42(&text);
    Line::from(vec![
        Span::styled(format!("  {:<10}", label), label_style),
        Span::styled("[".to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(padded, val_style),
        Span::styled("]".to_string(), Style::default().fg(Color::DarkGray)),
    ])
}

fn mcp_scope_line(focused: bool, scope: McpScope) -> Line<'static> {
    let label_style = if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
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
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(text, style));
        if i < scopes.len() - 1 {
            spans.push(Span::styled("  ".to_string(), Style::default()));
        }
    }
    spans.push(Span::styled(
        "  [←→]".to_string(),
        Style::default().fg(Color::DarkGray),
    ));
    Line::from(spans)
}

fn mcp_input_line_opt(
    label: &str,
    input: &TextInput,
    focused: bool,
    active: bool,
) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    let label_style = if focused && active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        dim
    };
    let val_style = if active { Style::default() } else { dim };
    let text = if active {
        input.display(focused)
    } else {
        "  (not used for this scope)".to_string()
    };
    let padded = pad42(&text);
    Line::from(vec![
        Span::styled(format!("  {:<10}", label), label_style),
        Span::styled("[".to_string(), dim),
        Span::styled(padded, val_style),
        Span::styled("]".to_string(), dim),
    ])
}

fn mcp_confirm_line(focused: bool) -> Line<'static> {
    if focused {
        Line::from(vec![
            Span::styled(
                "  ▶ ".to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "[ Add Server ]".to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  ← press Enter".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        Line::from(vec![
            Span::raw("    ".to_string()),
            Span::styled(
                "[ Add Server ]".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    }
}

// ── field select popup ────────────────────────────────────────────────────────

pub fn render_field_select(f: &mut Frame, area: Rect, state: &FieldSelectState) {
    let field_name = match state.field {
        LaunchField::Tenant => "Tenant",
        LaunchField::Project => "Project",
        LaunchField::Repository => "Repository",
        _ => "Select",
    };
    let title = format!("Select {field_name}");

    let filtered = state.filtered_options();
    let n_opts = filtered.len();
    let height = (n_opts as u16 + 6).min(20).max(8);
    let popup_area = centered_rect(44, height, area);

    let mut lines: Vec<Line> = vec![Line::from("")];

    // Filter line
    let filter_display = if state.filter.is_empty() {
        Span::styled("  (type to filter)", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(
            format!("  filter: {}", state.filter),
            Style::default().fg(Color::Yellow),
        )
    };
    lines.push(Line::from(filter_display));
    lines.push(Line::from(""));

    // Option list
    for (i, opt) in filtered.iter().enumerate() {
        let selected = i == state.cursor;
        let (bullet, style) = if selected {
            (
                "▶ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("  ", Style::default().fg(Color::White))
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {bullet}"), style),
            Span::styled(opt.to_string(), style),
        ]));
    }

    if n_opts == 0 {
        lines.push(Line::from(Span::styled(
            "  (no options found)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ↑↓ navigate  Enter select  Esc cancel",
        Style::default().fg(Color::DarkGray),
    )));

    render_popup(f, popup_area, &title, lines);
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

pub fn render_confirm_remove_mcp(f: &mut Frame, area: Rect, entry: &McpEntry) {
    let popup_area = centered_rect(56, 9, area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Remove "),
            Span::styled(
                format!("\"{}\"", entry.name),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!(" ({})?", entry.scope),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(
            format!("  {}", entry.command_display),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("        "),
            Span::styled(
                "[y] Remove".to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("        "),
            Span::styled(
                "[Esc/n] Cancel".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(""),
    ];

    render_popup(f, popup_area, "Remove MCP", lines);
}
