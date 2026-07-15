use crate::app::{App, ENGINES, LaunchField};

fn truncate_task(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
};

pub fn render(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let dim = app.palette.dim;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(dim))
        .title(Span::styled(" Launch ", Style::default().fg(dim)))
        .padding(Padding::uniform(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0] spacer
            Constraint::Length(1), // [1] Engine row
            Constraint::Length(1), // [2] Workspace row
            Constraint::Length(1), // [3] spacer
            Constraint::Length(3), // [4] Tenant
            Constraint::Length(3), // [5] Project
            Constraint::Length(3), // [6] Repo
            Constraint::Length(1), // [7] spacer
            Constraint::Length(1), // [8] Task (read-only)
            Constraint::Length(1), // [9] spacer
            Constraint::Length(1), // [10] NoTmux
            Constraint::Length(1), // [11] spacer
            Constraint::Length(1), // [12] Launch button
            Constraint::Min(0),    // [13] remaining
        ])
        .split(inner);

    render_engine_row(f, app, rows[1]);
    render_workspace_row(f, app, rows[2]);
    render_text_field(f, app, rows[4], LaunchField::Tenant);
    render_text_field(f, app, rows[5], LaunchField::Project);
    render_text_field(f, app, rows[6], LaunchField::Repository);
    render_task_row(f, app, rows[8]);
    render_notmux(f, app, rows[10]);
    render_launch_button(f, app, rows[12]);
}

fn render_engine_row(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let focused = app.launch.focused == LaunchField::Engine;
    let label_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(dim)
    };

    let mut spans = vec![Span::styled("  Engine:    ", label_style)];

    for (i, engine) in ENGINES.iter().enumerate() {
        let selected = i == app.launch.engine_idx;
        let (bullet, style) = if selected {
            (
                "● ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )
        } else {
            ("○ ", Style::default().fg(dim))
        };
        spans.push(Span::styled(bullet, style));
        spans.push(Span::styled(engine.as_str(), style));
        if i + 1 < ENGINES.len() {
            spans.push(Span::raw("   "));
        }
    }

    if focused {
        spans.push(Span::styled("  [←→]", Style::default().fg(dim)));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_workspace_row(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let focused = app.launch.focused == LaunchField::Workspace;
    let label_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(dim)
    };

    let ws_name = app.launch.workspace_name();
    let ws_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Reset)
    };

    let n = app.launch.workspaces.len();
    let mut spans = vec![
        Span::styled("  Workspace: ", label_style),
        Span::styled("● ", ws_style),
        Span::styled(
            if ws_name.is_empty() {
                "(none)".to_string()
            } else {
                ws_name.to_string()
            },
            ws_style,
        ),
    ];

    if n > 1 {
        spans.push(Span::styled(format!("  ({n})"), Style::default().fg(dim)));
    }

    if focused && n > 1 {
        spans.push(Span::styled("  [←→]", Style::default().fg(dim)));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_text_field(
    f: &mut Frame,
    app: &mut App,
    area: ratatui::layout::Rect,
    field: LaunchField,
) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let focused = app.launch.focused == field;

    let (label, display_text) = match field {
        LaunchField::Tenant => {
            let t = app.launch.tenant.display(focused);
            ("Tenant    ", t)
        }
        LaunchField::Project => {
            let t = app.launch.project.display(focused);
            ("Project   ", t)
        }
        LaunchField::Repository => {
            let t = app.launch.repository.display(focused);
            ("Repo      ", t)
        }
        _ => return,
    };

    let label_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(dim)
    };

    let border_style = if focused {
        Style::default().fg(accent)
    } else {
        Style::default().fg(dim)
    };

    let text_style = if focused {
        Style::default()
    } else if display_text.contains('\u{2502}') || display_text.is_empty() {
        Style::default().fg(dim)
    } else {
        Style::default()
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(12), Constraint::Min(20)])
        .split(area);

    let label_area = ratatui::layout::Rect {
        y: cols[0].y + 1,
        height: 1,
        ..cols[0]
    };
    f.render_widget(
        Paragraph::new(Span::styled(format!("  {label}"), label_style)).alignment(Alignment::Left),
        label_area,
    );

    let box_block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let box_area = ratatui::layout::Rect {
        width: cols[1].width.min(40),
        ..cols[1]
    };

    let inner = box_block.inner(box_area);
    f.render_widget(box_block, box_area);

    f.render_widget(
        Paragraph::new(Span::styled(display_text.clone(), text_style)),
        inner,
    );
}

fn render_task_row(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let focused = app.launch.focused == LaunchField::Task;
    let label_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(dim)
    };

    let mut spans = vec![Span::styled("  Task:      ", label_style)];

    match &app.launch.task_context {
        Some(task) => {
            let task_style = if focused {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Reset)
            };
            spans.push(Span::styled(
                format!("{}  {}", task.key, truncate_task(&task.summary, 40)),
                task_style,
            ));
            if focused {
                spans.push(Span::styled("  [Esc] clear", Style::default().fg(dim)));
            }
        }
        None => {
            spans.push(Span::styled("—", Style::default().fg(dim)));
            if focused && app.jira_enabled {
                spans.push(Span::styled("  [t] open Tasks", Style::default().fg(dim)));
            }
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_notmux(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let focused = app.launch.focused == LaunchField::NoTmux;
    let label_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(dim)
    };
    let check = if app.launch.no_tmux { "[✓]" } else { "[ ]" };
    let check_style = if app.launch.no_tmux {
        Style::default().fg(accent)
    } else {
        Style::default().fg(dim)
    };

    let line = Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(check, check_style),
        Span::styled(" No tmux ", label_style),
        Span::styled("(launch in current terminal)", Style::default().fg(dim)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_launch_button(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let focused = app.launch.focused == LaunchField::Launch;
    let (prefix, style) = if focused {
        (
            "  ▶ ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )
    } else {
        ("    ", Style::default().fg(dim))
    };

    let line = Line::from(vec![
        Span::styled(prefix, style),
        Span::styled("[ Launch ]", style),
        if focused {
            Span::styled("  ← press Enter", Style::default().fg(dim))
        } else {
            Span::raw("")
        },
    ]);
    f.render_widget(Paragraph::new(line), area);
}
