use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph},
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let dim = app.palette.dim;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim))
        .title(Span::styled(" System ", Style::default().fg(dim)))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let sections = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(0),
    ])
    .split(inner);

    render_info(f, app, sections[0]);
    render_mcp(f, app, sections[1]);
}

fn render_info(f: &mut Frame, app: &App, area: Rect) {
    let dim = app.palette.dim;
    let warning = app.palette.warning;
    let success = app.palette.success;
    let sys = &app.sys;

    let (dev_bullet, dev_label, dev_style) = if sys.dev_mode {
        (
            "●",
            " dev  (orbit → orbit-dev)",
            Style::default()
                .fg(warning)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("○", " stable", Style::default().fg(success))
    };

    let (d_bullet, d_label, d_style) = if sys.daemon_running {
        ("●", " running", Style::default().fg(success))
    } else {
        ("○", " stopped", Style::default().fg(dim))
    };

    let version = env!("CARGO_PKG_VERSION");

    let k = |s: &'static str| Span::styled(s, Style::default().fg(dim));

    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![
            k("  AI Root:  "),
            Span::styled(
                sys.ai_root.display().to_string(),
                Style::default().fg(Color::Reset),
            ),
            k("   Engine: "),
            Span::styled(
                sys.default_engine.clone(),
                Style::default().fg(Color::Reset),
            ),
        ]),
        Line::from(vec![
            k("  Install:  "),
            Span::styled(
                sys.install_dir.display().to_string(),
                Style::default().fg(Color::Reset),
            ),
            k("   Tenant: "),
            Span::styled(
                sys.default_tenant.clone(),
                Style::default().fg(Color::Reset),
            ),
        ]),
        Line::from(vec![
            k("  Dev:      "),
            Span::styled(dev_bullet, dev_style),
            Span::styled(dev_label, dev_style),
            k("         Daemon: "),
            Span::styled(d_bullet, d_style),
            Span::styled(d_label, d_style),
            k("  [s]"),
            k("   orbit "),
            Span::styled(format!("v{version}"), Style::default().fg(dim)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  ──────────────────────────────────────────────────────────────────────",
            Style::default().fg(dim),
        )),
    ];

    f.render_widget(Paragraph::new(lines), area);
}

fn render_mcp(f: &mut Frame, app: &App, area: Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let label = app.palette.label;
    if area.height < 2 {
        return;
    }

    let sections = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .split(area);

    let header_lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(
                "  MCP Servers",
                Style::default()
                    .fg(label)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("                                        "),
            Span::styled("[a]", Style::default().fg(accent)),
            Span::raw(" add  "),
            Span::styled("[x]", Style::default().fg(accent)),
            Span::raw(" remove"),
        ]),
        Line::from(Span::styled(
            "  SCOPE               NAME              COMMAND",
            Style::default()
                .fg(dim)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    f.render_widget(Paragraph::new(header_lines), sections[0]);

    let list_area = sections[1];

    if app.sys.mcp_entries.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No MCP servers configured. Press [a] to add one.",
                Style::default().fg(dim),
            ))),
            list_area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .sys
        .mcp_entries
        .iter()
        .map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {:<20}", truncate(&e.scope, 20)),
                    Style::default().fg(dim),
                ),
                Span::styled(
                    format!("{:<18}", truncate(&e.name, 18)),
                    Style::default().fg(Color::Reset),
                ),
                Span::styled(
                    truncate(&e.command_display, 36),
                    Style::default().fg(dim),
                ),
            ]))
        })
        .collect();

    let sel_bg = app.palette.selected_bg;
    let sel_fg = app.palette.selected_fg;
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(sel_bg)
                .fg(sel_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    let mut list_state = ListState::default();
    list_state.select(Some(app.sys.mcp_selected));

    f.render_stateful_widget(list, list_area, &mut list_state);
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let t: String = chars[..max.saturating_sub(2)].iter().collect();
        format!("{t}..")
    }
}
