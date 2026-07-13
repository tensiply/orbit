mod adf;
mod launch;
mod plans;
mod popup;
mod schedules;
mod scopes;
mod sessions;
mod system;
mod tasks;
mod workspaces;

use crate::app::{App, Mode, Tab};
use ratatui::{
    Frame,
    layout::{Constraint, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let full = f.area();
    let area = full.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    let chunks = ratatui::layout::Layout::vertical([
        Constraint::Length(1), // top spacer
        Constraint::Length(1), // tab bar
        Constraint::Length(1), // spacer between tab bar and panel
        Constraint::Min(0),    // panel content
        Constraint::Length(1), // footer
    ])
    .split(area);

    render_tab_bar(f, app, chunks[1]);

    match app.tab {
        Tab::Sessions => sessions::render(f, app, chunks[3]),
        Tab::Launch => launch::render(f, app, chunks[3]),
        Tab::Plans => plans::render(f, app, chunks[3]),
        Tab::System => system::render(f, app, chunks[3]),
        Tab::Tasks => tasks::render(f, app, chunks[3]),
        Tab::Schedules => schedules::render(f, app, chunks[3]),
        Tab::Scopes => scopes::render(f, app, chunks[3]),
        Tab::Workspaces => workspaces::render(f, app, chunks[3]),
    }

    render_footer(f, app, chunks[4]);

    // Popup overlays (rendered last so they appear on top)
    let p = app.palette.clone();
    match &app.mode {
        Mode::Help => popup::render_help(f, full, &p),
        Mode::ConfirmKill(s) => popup::render_confirm_kill(f, full, s.clone(), &p),
        Mode::SessionDetails(s) => popup::render_details(f, full, s.clone(), &p),
        Mode::AddMcp(state) => popup::render_add_mcp(f, full, state, &app.sys.default_tenant, &p),
        Mode::ConfirmRemoveMcp(entry) => popup::render_confirm_remove_mcp(f, full, entry, &p),
        Mode::FieldSelect(state) => popup::render_field_select(f, full, state, &p),
        Mode::TaskDetailsLoading => popup::render_task_details_loading(f, full, &p),
        Mode::TaskDetailsError(msg) => popup::render_task_details_error(f, full, msg, &p),
        Mode::TaskDetails(detail) => popup::render_task_details(f, full, detail, &p),
        Mode::AddComment(state) => popup::render_add_comment(f, full, state, &p),
        Mode::Normal => {}
    }
}

fn render_tab_bar(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let dim = app.palette.dim;
    let ws_name = app.active_workspace_name().to_string();
    let ws_hint = if app.workspaces.len() > 1 {
        Span::styled("[w]", Style::default().fg(accent))
    } else {
        Span::styled("[w]", Style::default().fg(dim))
    };

    let mut spans = vec![
        Span::raw(" "),
        tab_span("[1]", app.tab == Tab::Sessions, accent, dim),
        Span::styled(
            " Sessions  ",
            tab_label_style(app.tab == Tab::Sessions, dim),
        ),
        tab_span("[2]", app.tab == Tab::Launch, accent, dim),
        Span::styled(" Launch  ", tab_label_style(app.tab == Tab::Launch, dim)),
        tab_span("[3]", app.tab == Tab::Plans, accent, dim),
        Span::styled(" Plans  ", tab_label_style(app.tab == Tab::Plans, dim)),
        tab_span("[4]", app.tab == Tab::System, accent, dim),
        Span::styled(" System  ", tab_label_style(app.tab == Tab::System, dim)),
    ];

    if app.jira_enabled {
        spans.push(tab_span("[5]", app.tab == Tab::Tasks, accent, dim));
        spans.push(Span::styled(
            " Tasks  ",
            tab_label_style(app.tab == Tab::Tasks, dim),
        ));
    }

    spans.push(tab_span("[6]", app.tab == Tab::Schedules, accent, dim));
    spans.push(Span::styled(
        " Schedules  ",
        tab_label_style(app.tab == Tab::Schedules, dim),
    ));
    spans.push(tab_span("[7]", app.tab == Tab::Scopes, accent, dim));
    spans.push(Span::styled(
        " Scopes  ",
        tab_label_style(app.tab == Tab::Scopes, dim),
    ));
    spans.push(tab_span("[8]", app.tab == Tab::Workspaces, accent, dim));
    spans.push(Span::styled(
        " Workspaces  ",
        tab_label_style(app.tab == Tab::Workspaces, dim),
    ));

    let label = app.palette.label;
    spans.push(Span::styled("─  ", Style::default().fg(dim)));
    spans.push(ws_hint);
    spans.push(Span::styled(
        format!(" {ws_name}"),
        Style::default().fg(label),
    ));

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn tab_span(label: &'static str, active: bool, accent: Color, dim: Color) -> Span<'static> {
    if active {
        Span::styled(
            label,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(label, Style::default().fg(dim))
    }
}

fn tab_label_style(active: bool, dim: Color) -> Style {
    if active {
        Style::default()
    } else {
        Style::default().fg(dim)
    }
}

fn render_footer(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let accent = app.palette.accent;
    let warning = app.palette.warning;
    let line = if let Some(msg) = &app.status_msg {
        Line::from(vec![
            Span::styled(" ! ", Style::default().fg(warning)),
            Span::styled(msg.clone(), Style::default().fg(warning)),
        ])
    } else {
        match app.tab {
            Tab::Sessions => Line::from(vec![
                hint(" [Tab]", accent),
                Span::raw(" switch  "),
                hint("[↑↓/jk]", accent),
                Span::raw(" nav  "),
                hint("[a/↵]", accent),
                Span::raw(" attach  "),
                hint("[K]", accent),
                Span::raw(" kill  "),
                hint("[d]", accent),
                Span::raw(" details  "),
                hint("[c]", accent),
                Span::raw(" clean  "),
                hint("[w]", accent),
                Span::raw(" workspace  "),
                hint("[?]", accent),
                Span::raw(" help  "),
                hint("[q]", accent),
                Span::raw(" quit"),
            ]),
            Tab::Launch => Line::from(vec![
                hint(" [Tab]", accent),
                Span::raw(" switch  "),
                hint("[↑↓]", accent),
                Span::raw(" fields  "),
                hint("[←→]", accent),
                Span::raw(" engine  "),
                hint("[Space]", accent),
                Span::raw(" toggle  "),
                hint("[↵]", accent),
                Span::raw(" confirm/launch  "),
                hint("[Esc]", accent),
                Span::raw(" back"),
            ]),
            Tab::Plans => Line::from(vec![
                hint(" [Tab]", accent),
                Span::raw(" switch  "),
                hint("[↑↓/jk]", accent),
                Span::raw(" nav  "),
                hint("[a]", accent),
                Span::raw(" approve  "),
                hint("[x]", accent),
                Span::raw(" cancel  "),
                hint("[r]", accent),
                Span::raw(" refresh  "),
                hint("[q]", accent),
                Span::raw(" quit"),
            ]),
            Tab::System => Line::from(vec![
                hint(" [Tab]", accent),
                Span::raw(" switch  "),
                hint("[↑↓/jk]", accent),
                Span::raw(" MCP nav  "),
                hint("[a]", accent),
                Span::raw(" add  "),
                hint("[x]", accent),
                Span::raw(" remove  "),
                hint("[s]", accent),
                Span::raw(" daemon  "),
                hint("[w]", accent),
                Span::raw(" workspace  "),
                hint("[r]", accent),
                Span::raw(" refresh  "),
                hint("[q]", accent),
                Span::raw(" quit"),
            ]),
            Tab::Tasks => Line::from(vec![
                hint(" [Tab]", accent),
                Span::raw(" switch  "),
                hint("[↑↓/jk]", accent),
                Span::raw(" nav  "),
                hint("[←→]", accent),
                Span::raw(" org filter  "),
                hint("[↵]", accent),
                Span::raw(" launch  "),
                hint("[d]", accent),
                Span::raw(" details  "),
                hint("[e]", accent),
                Span::raw(" browser  "),
                hint("[r]", accent),
                Span::raw(" refresh  "),
                hint("[q]", accent),
                Span::raw(" quit"),
            ]),
            Tab::Schedules => Line::from(vec![
                hint(" [Tab]", accent),
                Span::raw(" switch  "),
                hint("[↑↓/jk]", accent),
                Span::raw(" nav  "),
                hint("[x]", accent),
                Span::raw(" cancel  "),
                hint("[R]", accent),
                Span::raw(" run now  "),
                hint("[r]", accent),
                Span::raw(" refresh  "),
                hint("[q]", accent),
                Span::raw(" quit"),
            ]),
            Tab::Scopes => Line::from(vec![
                hint(" [Tab]", accent),
                Span::raw(" switch  "),
                hint("[↑↓/jk]", accent),
                Span::raw(" select scope  "),
                hint("[r]", accent),
                Span::raw(" refresh  "),
                hint("[q]", accent),
                Span::raw(" quit"),
            ]),
            Tab::Workspaces => Line::from(vec![
                hint(" [Tab]", accent),
                Span::raw(" switch  "),
                hint("[↑↓/jk]", accent),
                Span::raw(" select workspace  "),
                hint("[r]", accent),
                Span::raw(" refresh  "),
                hint("[q]", accent),
                Span::raw(" quit"),
            ]),
        }
    };
    f.render_widget(Paragraph::new(line), area);
}

fn hint(s: &'static str, accent: Color) -> Span<'static> {
    Span::styled(s, Style::default().fg(accent))
}
